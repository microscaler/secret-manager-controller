//! # SOPS Key Management
//!
//! Handles loading, reloading, and watching SOPS private keys from Kubernetes secrets.

use crate::controller::reconciler::types::Reconciler;
use anyhow::Result;
use kube::Client;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Load SOPS private key from Kubernetes secret in controller namespace
/// Defaults to microscaler-system namespace
pub async fn load_sops_private_key(client: &Client) -> Result<Option<String>> {
    use k8s_openapi::api::core::v1::Secret;
    use kube::Api;

    // Use controller namespace (defaults to microscaler-system)
    // Can be overridden via POD_NAMESPACE environment variable
    let namespace =
        std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "microscaler-system".to_string());

    let secrets: Api<Secret> = Api::namespaced(client.clone(), &namespace);

    // Try to get the SOPS private key secret
    // Expected secret name: sops-private-key (or similar)
    let secret_names = vec!["sops-private-key", "sops-gpg-key", "gpg-key"];

    for secret_name in secret_names {
        match secrets.get(secret_name).await {
            Ok(secret) => {
                // Extract private key from secret data
                // The key might be in different fields: "private-key", "key", "gpg-key", etc.
                if let Some(ref data_map) = secret.data {
                    if let Some(data) = data_map
                        .get("private-key")
                        .or_else(|| data_map.get("key"))
                        .or_else(|| data_map.get("gpg-key"))
                    {
                        let key = String::from_utf8(data.0.clone())
                            .map_err(|e| anyhow::anyhow!("Failed to decode private key: {e}"))?;
                        info!("Loaded SOPS private key from secret: {}", secret_name);
                        return Ok(Some(key));
                    }
                }
            }
            Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
                // Try next secret name
            }
            Err(e) => {
                warn!("Failed to get secret {}: {}", secret_name, e);
            }
        }
    }

    warn!(
        "SOPS private key not found in {} namespace, SOPS decryption will be disabled",
        namespace
    );
    Ok(None)
}

/// Reload SOPS private key from Kubernetes secret
/// Called when the secret changes to hot-reload the key without restarting
pub async fn reload_sops_private_key(reconciler: &Reconciler) -> Result<()> {
    let new_key = load_sops_private_key(&reconciler.client).await?;
    let mut key_guard = reconciler.sops_private_key.lock().await;
    *key_guard = new_key;

    if key_guard.is_some() {
        info!("✅ Reloaded SOPS private key from Kubernetes secret");
    } else {
        warn!("SOPS private key secret not found, SOPS decryption will be disabled");
    }

    Ok(())
}

/// Reload SOPS private key from a specific namespace
/// Falls back to controller namespace if not found
pub async fn reload_sops_private_key_from_namespace(
    reconciler: &Reconciler,
    namespace: &str,
) -> Result<()> {
    use k8s_openapi::api::core::v1::Secret;
    use kube::Api;

    let secrets: Api<Secret> = Api::namespaced(reconciler.client.clone(), namespace);
    let secret_names = vec!["sops-private-key", "sops-gpg-key", "gpg-key"];

    for secret_name in secret_names {
        match secrets.get(secret_name).await {
            Ok(secret) => {
                if let Some(ref data_map) = secret.data {
                    if let Some(data) = data_map
                        .get("private-key")
                        .or_else(|| data_map.get("key"))
                        .or_else(|| data_map.get("gpg-key"))
                    {
                        let key = String::from_utf8(data.0.clone())
                            .map_err(|e| anyhow::anyhow!("Failed to decode private key: {e}"))?;
                        let mut key_guard = reconciler.sops_private_key.lock().await;
                        *key_guard = Some(key);
                        info!(
                            "✅ Reloaded SOPS private key from secret '{}/{}'",
                            namespace, secret_name
                        );
                        return Ok(());
                    }
                }
            }
            Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
                // Try next secret name
            }
            Err(e) => {
                warn!(
                    "Failed to get secret '{}/{}': {}",
                    namespace, secret_name, e
                );
            }
        }
    }

    // Fallback to controller namespace
    warn!(
        "SOPS private key not found in namespace {}, falling back to controller namespace",
        namespace
    );
    reload_sops_private_key(reconciler).await
}

/// Verify RBAC is properly configured for SOPS key watch
/// Checks that ClusterRole, ClusterRoleBinding, and ServiceAccount exist
/// Then tests actual API access to verify RBAC is propagated
pub async fn verify_rbac_for_sops_watch(client: &kube::Client) -> Result<()> {
    use k8s_openapi::api::core::v1::{Secret, ServiceAccount};
    use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding};
    use kube::Api;

    const EXPECTED_CLUSTER_ROLE: &str = "secret-manager-controller";
    const EXPECTED_SERVICE_ACCOUNT: &str = "secret-manager-controller";
    const EXPECTED_NAMESPACE: &str = "microscaler-system";

    // Check ClusterRole exists
    let cluster_roles: Api<ClusterRole> = Api::all(client.clone());
    match cluster_roles.get(EXPECTED_CLUSTER_ROLE).await {
        Ok(_) => {
            debug!("✅ ClusterRole '{}' exists", EXPECTED_CLUSTER_ROLE);
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "ClusterRole '{}' not found: {}. Install RBAC: kubectl apply -f config/rbac/clusterrole.yaml",
                EXPECTED_CLUSTER_ROLE,
                e
            ));
        }
    }

    // Check ClusterRoleBinding exists
    let cluster_role_bindings: Api<ClusterRoleBinding> = Api::all(client.clone());
    match cluster_role_bindings.get(EXPECTED_CLUSTER_ROLE).await {
        Ok(crb) => {
            // Verify it binds to the correct ClusterRole
            if crb.role_ref.name != EXPECTED_CLUSTER_ROLE {
                return Err(anyhow::anyhow!(
                    "ClusterRoleBinding '{}' binds to wrong ClusterRole: {} (expected: {})",
                    EXPECTED_CLUSTER_ROLE,
                    crb.role_ref.name,
                    EXPECTED_CLUSTER_ROLE
                ));
            }
            debug!(
                "✅ ClusterRoleBinding '{}' exists and binds ClusterRole '{}'",
                EXPECTED_CLUSTER_ROLE, EXPECTED_CLUSTER_ROLE
            );
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "ClusterRoleBinding '{}' not found: {}. Install RBAC: kubectl apply -f config/rbac/clusterrolebinding.yaml",
                EXPECTED_CLUSTER_ROLE,
                e
            ));
        }
    }

    // Check ServiceAccount exists
    let service_accounts: Api<ServiceAccount> = Api::namespaced(client.clone(), EXPECTED_NAMESPACE);
    match service_accounts.get(EXPECTED_SERVICE_ACCOUNT).await {
        Ok(_) => {
            debug!(
                "✅ ServiceAccount '{}/{}' exists",
                EXPECTED_NAMESPACE, EXPECTED_SERVICE_ACCOUNT
            );
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "ServiceAccount '{}/{}' not found: {}. Install RBAC: kubectl apply -f config/rbac/serviceaccount.yaml",
                EXPECTED_NAMESPACE,
                EXPECTED_SERVICE_ACCOUNT,
                e
            ));
        }
    }

    // Test actual API access to verify RBAC is propagated
    // This is the real test - can we actually list secrets?
    let secrets: Api<Secret> = Api::all(client.clone());
    match secrets
        .list(&kube::api::ListParams::default().limit(1))
        .await
    {
        Ok(_) => {
            debug!("✅ RBAC permissions verified - can list secrets across all namespaces");
            Ok(())
        }
        Err(e) => {
            // RBAC resources exist but permissions not propagated yet
            Err(anyhow::anyhow!(
                "RBAC resources exist but permissions not active: {}. This usually means RBAC was created after pod started. Restart the pod to pick up RBAC changes.",
                e
            ))
        }
    }
}

/// Start watching for SOPS private key secret changes across all namespaces
/// Spawns a background task that watches for secret updates and reloads the key
/// Watches all namespaces to detect SOPS secret changes in tilt, dev, stage, prod, etc.
pub fn start_sops_key_watch(reconciler: Arc<Reconciler>) {
    tokio::spawn(async move {
        use futures::pin_mut;
        use futures::StreamExt;
        use k8s_openapi::api::core::v1::Secret;
        use kube::Api;
        use kube_runtime::watcher;

        // Watch secrets across ALL namespaces to detect SOPS key changes everywhere
        let secrets: Api<Secret> = Api::all(reconciler.client.clone());

        // Watch for secrets matching SOPS key names
        let secret_names = vec!["sops-private-key", "sops-gpg-key", "gpg-key"];

        info!("Starting watch for SOPS private key secrets across all namespaces");

        // Verify RBAC is properly configured and propagated before starting watch
        // This provides clear diagnostics for SREs if RBAC is misconfigured
        // verify_rbac_for_sops_watch checks resources exist AND tests actual API access
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 10;
        const RETRY_DELAY_SECS: u64 = 1;

        loop {
            match verify_rbac_for_sops_watch(&reconciler.client).await {
                Ok(_) => {
                    info!("✅ RBAC verified and propagated - ClusterRole, ClusterRoleBinding, ServiceAccount exist and permissions are active");
                    break;
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= MAX_RETRIES {
                        error!(
                            "❌ RBAC verification failed after {} attempts ({}s): {}",
                            MAX_RETRIES,
                            MAX_RETRIES as u64 * RETRY_DELAY_SECS,
                            e
                        );
                        error!("🔍 SRE Diagnostics:");
                        error!("   1. Verify ClusterRole 'secret-manager-controller' exists:");
                        error!("      kubectl get clusterrole secret-manager-controller");
                        error!("   2. Verify ClusterRoleBinding exists and binds ServiceAccount:");
                        error!("      kubectl get clusterrolebinding secret-manager-controller -o yaml");
                        error!("   3. Verify ServiceAccount exists:");
                        error!(
                            "      kubectl get sa secret-manager-controller -n microscaler-system"
                        );
                        error!("   4. Verify ServiceAccount is bound to ClusterRole:");
                        error!("      kubectl auth can-i list secrets --as=system:serviceaccount:microscaler-system:secret-manager-controller --all-namespaces");
                        error!("   5. Check pod is using correct ServiceAccount:");
                        error!("      kubectl get pod -n microscaler-system -l app=secret-manager-controller -o jsonpath='{{{{.items[0].spec.serviceAccountName}}}}'");
                        error!("   6. If RBAC resources exist but permissions not active:");
                        error!(
                            "      - ClusterRoleBinding may have been created after pod started"
                        );
                        error!("      - Kubernetes API server cache may need refresh");
                        error!("      - ServiceAccount token may need regeneration");
                        error!("      Action: Restart the controller pod to pick up RBAC changes");
                        warn!("⚠️  SOPS key watch will not be started. Controller will still work but SOPS key changes won't be hot-reloaded.");
                        warn!("⚠️  Fix RBAC configuration and restart the controller to enable SOPS key hot-reloading.");
                        return;
                    }
                    if retry_count % 3 == 0 {
                        // Log every 3rd retry to avoid spam
                        warn!(
                            "⏳ Waiting for RBAC propagation (attempt {}/{}): {}",
                            retry_count, MAX_RETRIES, e
                        );
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
                }
            }
        }

        // Watch all secrets in all namespaces and filter for SOPS key names
        // watcher() returns a Stream - pin it to use with StreamExt
        let stream = watcher(secrets, watcher::Config::default());
        pin_mut!(stream);

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    // Match on Event variants - handle all variants including Init events
                    match event {
                        watcher::Event::Apply(secret) => {
                            let secret_name = secret.metadata.name.as_deref().unwrap_or("unknown");
                            let secret_namespace =
                                secret.metadata.namespace.as_deref().unwrap_or("unknown");

                            // Check if this is one of the SOPS key secrets
                            if secret_names.contains(&secret_name) {
                                info!(
                                    "SOPS private key secret '{}/{}' changed, reloading...",
                                    secret_namespace, secret_name
                                );
                                // Reload from the namespace where the secret changed
                                if let Err(e) = reload_sops_private_key_from_namespace(
                                    &reconciler,
                                    secret_namespace,
                                )
                                .await
                                {
                                    error!(
                                        "Failed to reload SOPS private key from namespace {}: {}",
                                        secret_namespace, e
                                    );
                                }
                            }
                        }
                        watcher::Event::Delete(secret) => {
                            let secret_name = secret.metadata.name.as_deref().unwrap_or("unknown");
                            let secret_namespace =
                                secret.metadata.namespace.as_deref().unwrap_or("unknown");
                            if secret_names.contains(&secret_name) {
                                warn!(
                                    "SOPS private key secret '{}/{}' was deleted",
                                    secret_namespace, secret_name
                                );
                                // Try to reload from controller namespace as fallback
                                if let Err(e) = reload_sops_private_key(&reconciler).await {
                                    warn!("Failed to reload SOPS private key from controller namespace: {}", e);
                                    // Clear the key if reload fails
                                    let mut key_guard = reconciler.sops_private_key.lock().await;
                                    *key_guard = None;
                                    warn!("SOPS private key cleared, decryption will be disabled");
                                }
                            }
                        }
                        watcher::Event::Init
                        | watcher::Event::InitApply(_)
                        | watcher::Event::InitDone => {
                            // Initial watch events - ignore, we already loaded the key at startup
                        }
                    }
                }
                Err(e) => {
                    warn!("Error watching SOPS key secrets: {}", e);
                    // Continue watching - errors are transient
                }
            }
        }

        warn!("SOPS key secret watch stream ended");
    });
}
