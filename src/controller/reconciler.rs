//! # Reconciler
//!
//! Core reconciliation logic for `SecretManagerConfig` resources.
//!
//! The reconciler:
//! - Watches `SecretManagerConfig` resources across all namespaces
//! - Fetches `GitRepository` or `Application` artifacts
//! - Processes application secret files or kustomize builds
//! - Syncs secrets to Google Cloud Secret Manager
//! - Updates resource status with reconciliation results
//!
//! ## Reconciliation Flow
//!
//! 1. Get source (`FluxCD` `GitRepository` or `ArgoCD` `Application`)
//! 2. Extract artifact path
//! 3. Choose mode:
//!    - **Kustomize Build Mode**: Run `kustomize build` and extract secrets
//!    - **Raw File Mode**: Parse `application.secrets.env` files directly
//! 4. Decrypt SOPS-encrypted files if needed
//! 5. Sync secrets to GCP Secret Manager
//! 6. Update status

use crate::provider::aws::AwsSecretManager;
use crate::provider::aws::AwsParameterStore;
use crate::provider::azure::AzureKeyVault;
use crate::provider::azure::AzureAppConfiguration;
use crate::provider::gcp::SecretManagerClient as GcpSecretManagerClient;
use crate::provider::{ConfigStoreProvider, SecretManagerProvider};
use crate::{
    observability, Condition, ProviderConfig, SecretManagerConfig, SecretManagerConfigStatus,
    SourceRef,
};
use crate::controller::parser;
use anyhow::{Context, Result};
use kube::Client;
use kube_runtime::controller::Action;
use md5;
use std::path::PathBuf;
use std::time::Instant;
use thiserror::Error;
use tracing::{error, info, warn};

/// Construct secret name with prefix, key, and suffix
/// Matches kustomize-google-secret-manager naming convention for drop-in replacement
///
/// Format: {prefix}-{key}-{suffix} (if both prefix and suffix exist)
///         {prefix}-{key} (if only prefix exists)
///         {key}-{suffix} (if only suffix exists)
///         {key} (if neither exists)
///
/// Invalid characters (`.`, `/`, etc.) are replaced with `_` to match GCP Secret Manager requirements
#[must_use]
#[allow(clippy::doc_markdown)]
#[cfg(test)]
pub fn construct_secret_name(prefix: Option<&str>, key: &str, suffix: Option<&str>) -> String {
    construct_secret_name_impl(prefix, key, suffix)
}

#[cfg(not(test))]
fn construct_secret_name(prefix: Option<&str>, key: &str, suffix: Option<&str>) -> String {
    construct_secret_name_impl(prefix, key, suffix)
}

fn construct_secret_name_impl(prefix: Option<&str>, key: &str, suffix: Option<&str>) -> String {
    let mut parts = Vec::new();

    if let Some(p) = prefix {
        if !p.is_empty() {
            parts.push(p);
        }
    }

    parts.push(key);

    if let Some(s) = suffix {
        if !s.is_empty() {
            // Strip leading dashes from suffix to avoid double dashes when joining
            let suffix_trimmed = s.trim_start_matches('-');
            if !suffix_trimmed.is_empty() {
                parts.push(suffix_trimmed);
            }
        }
    }

    let name = parts.join("-");
    sanitize_secret_name(&name)
}

/// Sanitize secret name to comply with GCP Secret Manager naming requirements
/// Replaces invalid characters (`.`, `/`, etc.) with `_`
/// Matches kustomize-google-secret-manager character sanitization behavior
#[must_use]
#[cfg(test)]
pub fn sanitize_secret_name(name: &str) -> String {
    sanitize_secret_name_impl(name)
}

#[cfg(not(test))]
fn sanitize_secret_name(name: &str) -> String {
    sanitize_secret_name_impl(name)
}

fn sanitize_secret_name_impl(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            // GCP Secret Manager allows: [a-zA-Z0-9_-]+
            // Replace common invalid characters with underscore
            '.' | '/' | ' ' => '_',
            // Keep valid characters
            c if c.is_alphanumeric() || c == '-' || c == '_' => c,
            // Replace any other invalid character with underscore
            _ => '_',
        })
        .collect();

    // Remove consecutive dashes (double dashes, triple dashes, etc.)
    // This handles cases where sanitization creates multiple dashes in a row
    let mut result = String::with_capacity(sanitized.len());
    let mut prev_was_dash = false;

    for c in sanitized.chars() {
        if c == '-' {
            if !prev_was_dash {
                result.push(c);
                prev_was_dash = true;
            }
        } else {
            result.push(c);
            prev_was_dash = false;
        }
    }

    // Remove leading and trailing dashes
    result.trim_matches('-').to_string()
}

#[derive(Debug, Error)]
pub enum ReconcilerError {
    #[error("Reconciliation failed: {0}")]
    ReconciliationFailed(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct Reconciler {
    client: Client,
    // Note: secret_manager is created per-reconciliation to support per-resource auth config
    // In the future, we might want to cache clients per auth config
    sops_private_key: Option<String>,
}

impl Reconciler {
    #[allow(clippy::missing_errors_doc)]
    pub async fn new(client: Client) -> Result<Self> {
        // Provider is created per-reconciliation based on provider config
        // Per-resource auth config is handled in reconcile()

        // Load SOPS private key from Kubernetes secret
        let sops_private_key = Self::load_sops_private_key(&client).await?;

        Ok(Self {
            client,
            sops_private_key,
        })
    }

    /// Load SOPS private key from Kubernetes secret in controller namespace
    /// Defaults to microscaler-system namespace
    async fn load_sops_private_key(client: &Client) -> Result<Option<String>> {
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
                            let key = String::from_utf8(data.0.clone()).map_err(|e| {
                                anyhow::anyhow!("Failed to decode private key: {e}")
                            })?;
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

    #[allow(clippy::too_many_lines, clippy::missing_errors_doc)]
    pub async fn reconcile(
        config: std::sync::Arc<SecretManagerConfig>,
        ctx: std::sync::Arc<Reconciler>,
    ) -> Result<Action, ReconcilerError> {
        // Wrap entire reconciliation in error handling to prevent panics
        match Self::reconcile_internal(config, ctx).await {
            Ok(action) => Ok(action),
            Err(e) => {
                error!("Reconciliation failed with error: {}", e);
                observability::metrics::increment_reconciliation_errors();
                Err(e)
            }
        }
    }

    #[allow(clippy::too_many_lines, clippy::missing_errors_doc)]
    async fn reconcile_internal(
        config: std::sync::Arc<SecretManagerConfig>,
        ctx: std::sync::Arc<Reconciler>,
    ) -> Result<Action, ReconcilerError> {
        let start = Instant::now();
        let name = config.metadata.name.as_deref().unwrap_or("unknown");

        // Validate required fields before proceeding
        if config.spec.source_ref.kind.is_empty() {
            let err = anyhow::anyhow!("sourceRef.kind is required but is empty");
            error!("Validation error for {}: {}", name, err);
            return Err(ReconcilerError::ReconciliationFailed(err));
        }

        if config.spec.source_ref.name.is_empty() {
            let err = anyhow::anyhow!("sourceRef.name is required but is empty");
            error!("Validation error for {}: {}", name, err);
            return Err(ReconcilerError::ReconciliationFailed(err));
        }

        if config.spec.source_ref.namespace.is_empty() {
            let err = anyhow::anyhow!("sourceRef.namespace is required but is empty");
            error!("Validation error for {}: {}", name, err);
            return Err(ReconcilerError::ReconciliationFailed(err));
        }

        if config.spec.secrets.environment.is_empty() {
            let err = anyhow::anyhow!("secrets.environment is required but is empty");
            error!("Validation error for {}: {}", name, err);
            return Err(ReconcilerError::ReconciliationFailed(err));
        }

        // Check if this is a manual reconciliation trigger (via annotation)
        let is_manual_trigger = config
            .metadata
            .annotations
            .as_ref()
            .and_then(|ann| ann.get("secret-management.microscaler.io/reconcile"))
            .is_some();

        if is_manual_trigger {
            info!(
                "Manual reconciliation triggered for SecretManagerConfig: {} (via msmctl CLI)",
                name
            );
        } else {
            info!("Reconciling SecretManagerConfig: {}", name);
        }

        observability::metrics::increment_reconciliations();

        // Validate and log SecretManagerConfig resource first
        info!(
            "📋 SecretManagerConfig resource details: name={}, namespace={}, sourceRef.kind={}, sourceRef.name={}, sourceRef.namespace={}",
            name,
            config.metadata.namespace.as_deref().unwrap_or("default"),
            config.spec.source_ref.kind,
            config.spec.source_ref.name,
            config.spec.source_ref.namespace
        );
        
        info!(
            "📋 Secrets config: environment={}, prefix={}, basePath={:?}",
            config.spec.secrets.environment,
            config.spec.secrets.prefix.as_deref().unwrap_or("none"),
            config.spec.secrets.base_path
        );
        
        info!(
            "📋 Provider config: type={:?}",
            match &config.spec.provider {
                ProviderConfig::Gcp(_) => "gcp",
                ProviderConfig::Aws(_) => "aws",
                ProviderConfig::Azure(_) => "azure",
            }
        );

        // Get source and artifact path based on source type
        info!("🔍 Checking source: {} '{}' in namespace '{}'", 
            config.spec.source_ref.kind,
            config.spec.source_ref.name,
            config.spec.source_ref.namespace
        );
        
        let artifact_path = match config.spec.source_ref.kind.as_str() {
            "GitRepository" => {
                // FluxCD GitRepository - get artifact path from status
                info!("📦 Fetching FluxCD GitRepository: {}/{}", 
                    config.spec.source_ref.namespace,
                    config.spec.source_ref.name
                );
                
                let git_repo = match Reconciler::get_flux_git_repository(
                    &ctx,
                    &config.spec.source_ref,
                )
                .await
                {
                    Ok(repo) => {
                        info!("✅ Successfully retrieved GitRepository: {}/{}", 
                            config.spec.source_ref.namespace,
                            config.spec.source_ref.name
                        );
                        repo
                    }
                    Err(e) => {
                        // Check if this is a 404 (resource not found) - this is expected and we should wait
                        // The error is wrapped in anyhow::Error, so we need to check the root cause
                        let is_404 = e
                            .chain()
                            .any(|err| {
                                if let Some(kube_err) = err.downcast_ref::<kube::Error>() {
                                    if let kube::Error::Api(api_err) = kube_err {
                                        return api_err.code == 404;
                                    }
                                }
                                false
                            });

                        if is_404 {
                            warn!(
                                "⏳ GitRepository {}/{} not found yet, will retry in 30s",
                                config.spec.source_ref.namespace,
                                config.spec.source_ref.name
                            );
                            // Return requeue action - don't treat as error, just wait for resource
                            return Ok(Action::requeue(std::time::Duration::from_secs(30)));
                        }

                        // For other errors, log and fail
                        error!("❌ Failed to get FluxCD GitRepository: {}/{} - {}", 
                            config.spec.source_ref.namespace,
                            config.spec.source_ref.name,
                            e
                        );
                        observability::metrics::increment_reconciliation_errors();
                        return Err(ReconcilerError::ReconciliationFailed(e));
                    }
                };

                match Reconciler::get_flux_artifact_path(&ctx, &git_repo) {
                    Ok(path) => {
                        info!(
                            "Found FluxCD artifact path: {} for GitRepository: {}",
                            path.display(),
                            config.spec.source_ref.name
                        );
                        path
                    }
                    Err(e) => {
                        error!("Failed to get FluxCD artifact path: {}", e);
                        observability::metrics::increment_reconciliation_errors();
                        return Err(ReconcilerError::ReconciliationFailed(e));
                    }
                }
            }
            "Application" => {
                // ArgoCD Application - get Git source and clone/access repository
                match Reconciler::get_argocd_artifact_path(&ctx, &config.spec.source_ref).await {
                    Ok(path) => {
                        info!(
                            "Found ArgoCD artifact path: {} for Application: {}",
                            path.display(),
                            config.spec.source_ref.name
                        );
                        path
                    }
                    Err(e) => {
                        error!("Failed to get ArgoCD artifact path: {}", e);
                        observability::metrics::increment_reconciliation_errors();
                        return Err(ReconcilerError::ReconciliationFailed(e));
                    }
                }
            }
            _ => {
                error!("Unsupported source kind: {}", config.spec.source_ref.kind);
                observability::metrics::increment_reconciliation_errors();
                return Err(ReconcilerError::ReconciliationFailed(anyhow::anyhow!(
                    "Unsupported source kind: {}",
                    config.spec.source_ref.kind
                )));
            }
        };

        // Create provider based on provider config
        let provider: Box<dyn SecretManagerProvider> = match &config.spec.provider {
            ProviderConfig::Gcp(gcp_config) => {
                // Validate GCP config
                if gcp_config.project_id.is_empty() {
                    let err = anyhow::anyhow!("GCP projectId is required but is empty");
                    error!("Validation error for {}: {}", name, err);
                    return Err(ReconcilerError::ReconciliationFailed(err));
                }

                // Determine authentication method from config
                // Default to Workload Identity when auth is not specified
                let (auth_type, service_account_email_owned) =
                    if let Some(ref auth_config) = gcp_config.auth {
                        match serde_json::to_value(auth_config)
                            .context("Failed to serialize gcpAuth config")
                        {
                            Ok(auth_json) => {
                                let auth_type_str = auth_json.get("authType").and_then(|t| t.as_str());
                                if let Some("WorkloadIdentity") = auth_type_str {
                                    match auth_json
                                        .get("serviceAccountEmail")
                                        .and_then(|e| e.as_str())
                                    {
                                        Some(email) => (Some("WorkloadIdentity"), Some(email.to_string())),
                                        None => {
                                            warn!("WorkloadIdentity specified but serviceAccountEmail is missing, using default");
                                            (Some("WorkloadIdentity"), None)
                                        }
                                    }
                                } else {
                                    // Default to Workload Identity
                                    info!("No auth type specified, defaulting to Workload Identity");
                                    (Some("WorkloadIdentity"), None)
                                }
                            }
                            Err(e) => {
                                error!("Failed to serialize GCP auth config: {}", e);
                                return Err(ReconcilerError::ReconciliationFailed(e));
                            }
                        }
                    } else {
                        // Default to Workload Identity when auth is not specified
                        info!("No auth configuration specified, defaulting to Workload Identity");
                        (Some("WorkloadIdentity"), None)
                    };

                let service_account_email = service_account_email_owned.as_deref();
                match GcpSecretManagerClient::new(
                    gcp_config.project_id.clone(),
                    auth_type,
                    service_account_email,
                )
                .await
                {
                    Ok(gcp_client) => Box::new(gcp_client),
                    Err(e) => {
                        error!("Failed to create GCP Secret Manager client: {}", e);
                        return Err(ReconcilerError::ReconciliationFailed(e));
                    }
                }
            }
            ProviderConfig::Aws(aws_config) => {
                match AwsSecretManager::new(aws_config, &ctx.client).await {
                    Ok(aws_provider) => Box::new(aws_provider),
                    Err(e) => {
                        error!("Failed to create AWS Secrets Manager client: {}", e);
                        return Err(ReconcilerError::ReconciliationFailed(
                            e.context("Failed to create AWS Secrets Manager client")
                        ));
                    }
                }
            }
            ProviderConfig::Azure(azure_config) => {
                match AzureKeyVault::new(azure_config, &ctx.client).await {
                    Ok(azure_provider) => Box::new(azure_provider),
                    Err(e) => {
                        error!("Failed to create Azure Key Vault client: {}", e);
                        return Err(ReconcilerError::ReconciliationFailed(
                            e.context("Failed to create Azure Key Vault client")
                        ));
                    }
                }
            }
        };

        let mut secrets_synced = 0;

        // Check if kustomize_path is specified - use kustomize build mode
        if let Some(kustomize_path) = &config.spec.secrets.kustomize_path {
            // Use kustomize build to extract secrets from generated Secret resources
            // This supports overlays, patches, and generators
            info!("Using kustomize build mode on path: {}", kustomize_path);

            match crate::controller::kustomize::extract_secrets_from_kustomize(&artifact_path, kustomize_path) {
                Ok(secrets) => {
                    let secret_prefix = config.spec.secrets.prefix.as_deref().unwrap_or("default");
                    match ctx
                        .process_kustomize_secrets(&*provider, &config, &secrets, secret_prefix)
                        .await
                    {
                        Ok(count) => {
                            secrets_synced += count;
                            info!("Synced {} secrets from kustomize build", count);
                        }
                        Err(e) => {
                            error!("Failed to process kustomize secrets: {}", e);
                            observability::metrics::increment_reconciliation_errors();
                            return Err(ReconcilerError::ReconciliationFailed(e));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to extract secrets from kustomize build: {}", e);
                    observability::metrics::increment_reconciliation_errors();
                    return Err(ReconcilerError::ReconciliationFailed(e));
                }
            }
        } else {
            // Use raw file mode - read application.secrets.env files directly
            info!("Using raw file mode");

            // Find application files for the specified environment
            // Pass secret_prefix as default_service_name for single service deployments
            let default_service_name = config.spec.secrets.prefix.as_deref();
            let application_files = match parser::find_application_files(
                &artifact_path,
                config.spec.secrets.base_path.as_deref(),
                &config.spec.secrets.environment,
                default_service_name,
            )
            .await
            {
                Ok(files) => files,
                Err(e) => {
                    error!(
                        "Failed to find application files for environment '{}': {}",
                        config.spec.secrets.environment, e
                    );
                    observability::metrics::increment_reconciliation_errors();
                    return Err(ReconcilerError::ReconciliationFailed(e));
                }
            };

            info!("Found {} application file sets", application_files.len());

            // Process each application file set
            for app_files in application_files {
                match ctx
                    .process_application_files(&*provider, &config, &app_files, &ctx)
                    .await
                {
                    Ok(count) => {
                        secrets_synced += count;
                        info!("Synced {} secrets for {}", count, app_files.service_name);
                    }
                    Err(e) => {
                        error!("Failed to process {}: {}", app_files.service_name, e);
                    }
                }
            }
        }

        // Update status
        if let Err(e) = ctx.update_status(&config, secrets_synced).await {
            error!("Failed to update status: {}", e);
            observability::metrics::increment_reconciliation_errors();
            return Err(ReconcilerError::ReconciliationFailed(e));
        }

        // Update metrics
        observability::metrics::observe_reconciliation_duration(start.elapsed().as_secs_f64());
        observability::metrics::set_secrets_managed(i64::from(secrets_synced));

        info!(
            "Reconciliation complete for {} (synced {} secrets)",
            name, secrets_synced
        );
        Ok(Action::await_change())
    }

    /// Get FluxCD GitRepository resource
    #[allow(clippy::doc_markdown, clippy::missing_errors_doc)]
    async fn get_flux_git_repository(&self, source_ref: &SourceRef) -> Result<serde_json::Value> {
        // Use Kubernetes API to get GitRepository
        // GitRepository is a CRD from source.toolkit.fluxcd.io/v1beta2
        use kube::api::ApiResource;
        use kube::core::DynamicObject;

        let ar = ApiResource::from_gvk(&kube::core::GroupVersionKind {
            group: "source.toolkit.fluxcd.io".to_string(),
            version: "v1beta2".to_string(),
            kind: "GitRepository".to_string(),
        });

        let api: kube::Api<DynamicObject> =
            kube::Api::namespaced_with(self.client.clone(), &source_ref.namespace, &ar);

        let git_repo = api.get(&source_ref.name).await.context(format!(
            "Failed to get FluxCD GitRepository: {}/{}",
            source_ref.namespace, source_ref.name
        ))?;

        Ok(serde_json::to_value(git_repo)?)
    }

    /// Get artifact path from FluxCD GitRepository status
    #[allow(clippy::doc_markdown, clippy::unused_async, clippy::missing_errors_doc, clippy::unused_self)]
    fn get_flux_artifact_path(&self, git_repo: &serde_json::Value) -> Result<PathBuf> {
        // Extract artifact path from GitRepository status
        // Flux stores artifacts at: /tmp/flux-source-<namespace>-<name>-<revision>
        // We can also get it from status.artifact.url or status.artifact.path

        let status = git_repo
            .get("status")
            .and_then(|s| s.get("artifact"))
            .context("FluxCD GitRepository has no artifact in status")?;

        // Try to get path from artifact
        if let Some(path) = status.get("path").and_then(|p| p.as_str()) {
            return Ok(PathBuf::from(path));
        }

        // Fallback: construct path from GitRepository metadata
        let metadata = git_repo
            .get("metadata")
            .context("FluxCD GitRepository has no metadata")?;

        let name = metadata
            .get("name")
            .and_then(|n| n.as_str())
            .context("FluxCD GitRepository has no name")?;

        let namespace = metadata
            .get("namespace")
            .and_then(|n| n.as_str())
            .context("FluxCD GitRepository has no namespace")?;

        // Default Flux artifact path
        let default_path = format!("/tmp/flux-source-{namespace}-{name}");
        warn!("Using default FluxCD artifact path: {}", default_path);
        Ok(PathBuf::from(default_path))
    }

    /// Get artifact path from ArgoCD Application
    /// Clones the Git repository directly from the Application spec
    #[allow(clippy::doc_markdown, clippy::missing_errors_doc, clippy::unused_async, clippy::too_many_lines)]
    async fn get_argocd_artifact_path(&self, source_ref: &SourceRef) -> Result<PathBuf> {
        use kube::api::ApiResource;
        use kube::core::DynamicObject;

        // Get ArgoCD Application CRD
        // Application is from argoproj.io/v1alpha1
        let ar = ApiResource::from_gvk(&kube::core::GroupVersionKind {
            group: "argoproj.io".to_string(),
            version: "v1alpha1".to_string(),
            kind: "Application".to_string(),
        });

        let api: kube::Api<DynamicObject> =
            kube::Api::namespaced_with(self.client.clone(), &source_ref.namespace, &ar);

        let application = api.get(&source_ref.name).await.context(format!(
            "Failed to get ArgoCD Application: {}/{}",
            source_ref.namespace, source_ref.name
        ))?;

        // Extract Git source from Application spec
        let spec = application
            .data
            .get("spec")
            .context("ArgoCD Application has no spec")?;

        let source = spec
            .get("source")
            .context("ArgoCD Application has no source in spec")?;

        let repo_url = source
            .get("repoURL")
            .and_then(|u| u.as_str())
            .context("ArgoCD Application source has no repoURL")?;

        let target_revision = source
            .get("targetRevision")
            .and_then(|r| r.as_str())
            .unwrap_or("HEAD");

        info!(
            "ArgoCD Application source: repo={}, revision={}",
            repo_url, target_revision
        );

        // Clone repository to temporary directory
        // Use a deterministic path based on Application name/namespace/revision for caching
        let repo_hash = format!(
            "{:x}",
            md5::compute(format!(
                "{}-{}-{}",
                source_ref.namespace, source_ref.name, target_revision
            ))
        );
        let clone_path = format!("/tmp/argocd-repo-{repo_hash}");
        let path_buf = PathBuf::from(&clone_path);

        // Check if repository already exists and is at the correct revision
        if path_buf.exists() {
            // Verify the revision matches by checking HEAD
            let git_dir = path_buf.join(".git");
            if git_dir.exists() || path_buf.join("HEAD").exists() {
                // Check current HEAD revision
                let output = tokio::process::Command::new("git")
                    .arg("-C")
                    .arg(&path_buf)
                    .arg("rev-parse")
                    .arg("HEAD")
                    .output()
                    .await;

                if let Ok(output) = output {
                    if output.status.success() {
                        let current_rev =
                            String::from_utf8_lossy(&output.stdout).trim().to_string();
                        // Try to resolve target revision
                        let target_output = tokio::process::Command::new("git")
                            .arg("-C")
                            .arg(&path_buf)
                            .arg("rev-parse")
                            .arg(target_revision)
                            .output()
                            .await;

                        if let Ok(target_output) = target_output {
                            if target_output.status.success() {
                                let target_rev = String::from_utf8_lossy(&target_output.stdout)
                                    .trim()
                                    .to_string();
                                if current_rev == target_rev {
                                    info!(
                                        "Using cached ArgoCD repository at {} (revision: {})",
                                        clone_path, target_revision
                                    );
                                    return Ok(path_buf);
                                }
                            }
                        }
                    }
                }
            }
            // Remove stale repository
            if let Err(e) = tokio::fs::remove_dir_all(&path_buf).await {
                warn!("Failed to remove stale repository at {}: {}", clone_path, e);
            }
        }

        // Clone the repository using git command
        info!(
            "Cloning ArgoCD repository: {} (revision: {})",
            repo_url, target_revision
        );

        // Create parent directory
        let parent_dir = path_buf.parent().ok_or_else(|| {
            anyhow::anyhow!("Cannot determine parent directory for path: {}", clone_path)
        })?;
        tokio::fs::create_dir_all(parent_dir).await
            .context(format!("Failed to create parent directory for {}", clone_path))?;

        // Clone repository (shallow clone for efficiency)
        // First try shallow clone with branch (works for branch/tag names)
        let clone_output = tokio::process::Command::new("git")
            .arg("clone")
            .arg("--depth")
            .arg("1")
            .arg("--branch")
            .arg(target_revision)
            .arg(repo_url)
            .arg(&clone_path)
            .output()
            .await
            .context(format!("Failed to execute git clone for {repo_url}"))?;

        if !clone_output.status.success() {
            // If branch clone fails, clone default branch and checkout specific revision
            // This handles commit SHAs and other revision types
            let clone_output = tokio::process::Command::new("git")
                .arg("clone")
                .arg("--depth")
                .arg("50") // Deeper clone to ensure revision is available
                .arg(repo_url)
                .arg(&clone_path)
                .output()
                .await
                .context(format!("Failed to execute git clone for {repo_url}"))?;

            if !clone_output.status.success() {
                let error_msg = String::from_utf8_lossy(&clone_output.stderr);
                return Err(anyhow::anyhow!(
                    "Failed to clone repository {repo_url}: {error_msg}"
                ));
            }

            // Fetch the specific revision if needed
            let _fetch_output = tokio::process::Command::new("git")
                .arg("-C")
                .arg(&clone_path)
                .arg("fetch")
                .arg("--depth")
                .arg("50")
                .arg("origin")
                .arg(target_revision)
                .output()
                .await;

            // Checkout specific revision
            let checkout_output = tokio::process::Command::new("git")
                .arg("-C")
                .arg(&clone_path)
                .arg("checkout")
                .arg(target_revision)
                .output()
                .await
                .context(format!(
                    "Failed to checkout revision {target_revision} in repository {repo_url}"
                ))?;

            if !checkout_output.status.success() {
                let error_msg = String::from_utf8_lossy(&checkout_output.stderr);
                return Err(anyhow::anyhow!(
                    "Failed to checkout revision {target_revision} in repository {repo_url}: {error_msg}"
                ));
            }
        }

        info!(
            "Successfully cloned ArgoCD repository to {} (revision: {})",
            clone_path, target_revision
        );
        Ok(path_buf)
    }

    #[allow(clippy::too_many_lines, clippy::unused_async)]
    async fn process_application_files(
        &self,
        provider: &dyn SecretManagerProvider,
        config: &SecretManagerConfig,
        app_files: &parser::ApplicationFiles,
        ctx: &Reconciler,
    ) -> Result<i32> {
        let secret_prefix = config
            .spec
            .secrets
            .prefix
            .as_deref()
            .unwrap_or(&app_files.service_name);

        // Parse secrets from files (with SOPS decryption if needed)
        let secrets = parser::parse_secrets(app_files, self.sops_private_key.as_deref()).await?;
        let properties = parser::parse_properties(app_files).await?;

        // Store secrets in cloud provider (GitOps: Git is source of truth)
        let mut count = 0;
        let mut updated_count = 0;

        for (key, value) in secrets {
            let secret_name = construct_secret_name(
                Some(secret_prefix),
                key.as_str(),
                config.spec.secrets.suffix.as_deref(),
            );
            match provider.create_or_update_secret(&secret_name, &value).await {
                Ok(was_updated) => {
                    count += 1;
                    if was_updated {
                        updated_count += 1;
                        info!(
                            "Updated secret {} from git (GitOps source of truth)",
                            secret_name
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to store secret {}: {}", secret_name, e);
                    return Err(e.context(format!("Failed to store secret: {secret_name}")));
                }
            }
        }

        if updated_count > 0 {
            observability::metrics::increment_secrets_updated(i64::from(updated_count));
            warn!(
                "Updated {} secrets from git (GitOps source of truth). Manual changes in cloud provider were overwritten.",
                updated_count
            );
        }

        // Store properties - route to config store if enabled, otherwise store as JSON blob in secret store
        if !properties.is_empty() {
            let configs_enabled = config
                .spec
                .configs
                .as_ref()
                .map(|c| c.enabled)
                .unwrap_or(false);

            if configs_enabled {
                // Route properties to config store (store individually)
                info!(
                    "Config store enabled: storing {} properties individually",
                    properties.len()
                );
                let mut config_count = 0;
                let mut config_updated_count = 0;

                // Route to appropriate config store based on provider
                match &config.spec.provider {
                    ProviderConfig::Gcp(_gcp_config) => {
                        // For GCP, reuse Secret Manager provider (store configs as individual secrets)
                        // This is an interim solution until Parameter Manager support is contributed to ESO
                        for (key, value) in properties {
                            let config_name = construct_secret_name(
                                Some(secret_prefix),
                                key.as_str(),
                                config.spec.secrets.suffix.as_deref(),
                            );
                            match provider.create_or_update_secret(&config_name, &value).await {
                                Ok(was_updated) => {
                                    config_count += 1;
                                    if was_updated {
                                        config_updated_count += 1;
                                        info!(
                                            "Updated config {} from git (GitOps source of truth)",
                                            config_name
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to store config {}: {}", config_name, e);
                                    return Err(e.context(format!("Failed to store config: {config_name}")));
                                }
                            }
                        }
                    }
                    ProviderConfig::Aws(aws_config) => {
                        // For AWS, use Parameter Store
                        let parameter_path = config
                            .spec
                            .configs
                            .as_ref()
                            .and_then(|c| c.parameter_path.as_deref());
                        let aws_param_store = AwsParameterStore::new(
                            aws_config,
                            parameter_path,
                            secret_prefix,
                            &config.spec.secrets.environment,
                            &ctx.client,
                        )
                        .await
                        .context("Failed to create AWS Parameter Store client")?;

                        for (key, value) in properties {
                            match aws_param_store.create_or_update_config(&key, &value).await {
                                Ok(was_updated) => {
                                    config_count += 1;
                                    if was_updated {
                                        config_updated_count += 1;
                                        info!(
                                            "Updated config {} from git (GitOps source of truth)",
                                            key
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to store config {}: {}", key, e);
                                    return Err(e.context(format!("Failed to store config: {key}")));
                                }
                            }
                        }
                    }
                    ProviderConfig::Azure(azure_config) => {
                        // For Azure, use App Configuration
                        let app_config_endpoint = config
                            .spec
                            .configs
                            .as_ref()
                            .and_then(|c| c.app_config_endpoint.as_deref());
                        let azure_app_config = AzureAppConfiguration::new(
                            azure_config,
                            app_config_endpoint,
                            secret_prefix,
                            &config.spec.secrets.environment,
                            &ctx.client,
                        )
                        .await
                        .context("Failed to create Azure App Configuration client")?;

                        for (key, value) in properties {
                            match azure_app_config.create_or_update_config(&key, &value).await {
                                Ok(was_updated) => {
                                    config_count += 1;
                                    if was_updated {
                                        config_updated_count += 1;
                                        info!(
                                            "Updated config {} from git (GitOps source of truth)",
                                            key
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to store config {}: {}", key, e);
                                    return Err(e.context(format!("Failed to store config: {key}")));
                                }
                            }
                        }
                    }
                }

                count += config_count;
                if config_updated_count > 0 {
                    observability::metrics::increment_secrets_updated(i64::from(config_updated_count));
                    warn!(
                        "Updated {} configs from git (GitOps source of truth). Manual changes in cloud provider were overwritten.",
                        config_updated_count
                    );
                }
            } else {
                // Backward compatibility: store properties as a single secret (JSON encoded)
                let properties_json = serde_json::to_string(&properties)?;
                let secret_name = construct_secret_name(
                    Some(secret_prefix),
                    "properties",
                    config.spec.secrets.suffix.as_deref(),
                );
                match provider
                    .create_or_update_secret(&secret_name, &properties_json)
                    .await
                {
                    Ok(was_updated) => {
                        count += 1;
                        if was_updated {
                            observability::metrics::increment_secrets_updated(1);
                            info!("Updated properties secret {} from git", secret_name);
                        }
                    }
                    Err(e) => {
                        error!("Failed to store properties: {}", e);
                        return Err(e.context("Failed to store properties"));
                    }
                }
            }
        }

        observability::metrics::increment_secrets_synced(i64::from(count));
        Ok(count)
    }

    async fn process_kustomize_secrets(
        &self,
        provider: &dyn SecretManagerProvider,
        config: &SecretManagerConfig,
        secrets: &std::collections::HashMap<String, String>,
        secret_prefix: &str,
    ) -> Result<i32> {
        // Store secrets in cloud provider (GitOps: Git is source of truth)
        let mut count = 0;
        let mut updated_count = 0;

        for (key, value) in secrets {
            let secret_name = construct_secret_name(
                Some(secret_prefix),
                key.as_str(),
                config.spec.secrets.suffix.as_deref(),
            );
            match provider.create_or_update_secret(&secret_name, value).await {
                Ok(was_updated) => {
                    count += 1;
                    if was_updated {
                        updated_count += 1;
                        info!(
                            "Updated secret {} from kustomize build (GitOps source of truth)",
                            secret_name
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to store secret {}: {}", secret_name, e);
                    return Err(e.context(format!("Failed to store secret: {secret_name}")));
                }
            }
        }

        if updated_count > 0 {
            observability::metrics::increment_secrets_updated(i64::from(updated_count));
            warn!(
                "Updated {} secrets from kustomize build (GitOps source of truth). Manual changes in cloud provider were overwritten.",
                updated_count
            );
        }

        observability::metrics::increment_secrets_synced(i64::from(count));
        Ok(count)
    }

    async fn update_status(&self, config: &SecretManagerConfig, secrets_synced: i32) -> Result<()> {
        use kube::api::PatchParams;

        let api: kube::Api<SecretManagerConfig> = kube::Api::namespaced(
            self.client.clone(),
            config.metadata.namespace.as_deref().unwrap_or("default"),
        );

        let status = SecretManagerConfigStatus {
            conditions: vec![Condition {
                r#type: "Ready".to_string(),
                status: "True".to_string(),
                last_transition_time: Some(chrono::Utc::now().to_rfc3339()),
                reason: Some("ReconciliationSucceeded".to_string()),
                message: Some(format!("Synced {secrets_synced} secrets")),
            }],
            observed_generation: config.metadata.generation,
            last_reconcile_time: Some(chrono::Utc::now().to_rfc3339()),
            secrets_synced: Some(secrets_synced),
        };

        let patch = serde_json::json!({
            "status": status
        });

        api.patch_status(
            config.metadata.name.as_deref().unwrap_or("unknown"),
            &PatchParams::apply("secret-manager-controller"),
            &kube::api::Patch::Merge(patch),
        )
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod secret_name_tests {
        use super::*;

        #[test]
        fn test_construct_secret_name_with_prefix_and_suffix() {
            let result = construct_secret_name(Some("my-service"), "database-url", Some("-prod"));
            assert_eq!(result, "my-service-database-url-prod"); // Leading dash stripped from suffix
        }

        #[test]
        fn test_construct_secret_name_with_prefix_only() {
            let result = construct_secret_name(Some("my-service"), "database-url", None);
            assert_eq!(result, "my-service-database-url");
        }

        #[test]
        fn test_construct_secret_name_with_suffix_only() {
            let result = construct_secret_name(None, "database-url", Some("-prod"));
            assert_eq!(result, "database-url-prod"); // Leading dash stripped from suffix
        }

        #[test]
        fn test_construct_secret_name_no_prefix_no_suffix() {
            let result = construct_secret_name(None, "database-url", None);
            assert_eq!(result, "database-url");
        }

        #[test]
        fn test_construct_secret_name_empty_prefix() {
            let result = construct_secret_name(Some(""), "database-url", Some("-prod"));
            assert_eq!(result, "database-url-prod"); // Leading dash stripped from suffix
        }

        #[test]
        fn test_construct_secret_name_empty_suffix() {
            let result = construct_secret_name(Some("my-service"), "database-url", Some(""));
            assert_eq!(result, "my-service-database-url");
        }

        #[test]
        fn test_construct_secret_name_properties_key() {
            let result = construct_secret_name(Some("my-service"), "properties", Some("-prod"));
            assert_eq!(result, "my-service-properties-prod"); // Leading dash stripped from suffix
        }

        #[test]
        fn test_sanitize_secret_name_dots() {
            let result = sanitize_secret_name("my.service.database.url");
            assert_eq!(result, "my_service_database_url");
        }

        #[test]
        fn test_sanitize_secret_name_slashes() {
            let result = sanitize_secret_name("my/service/database/url");
            assert_eq!(result, "my_service_database_url");
        }

        #[test]
        fn test_sanitize_secret_name_spaces() {
            let result = sanitize_secret_name("my service database url");
            assert_eq!(result, "my_service_database_url");
        }

        #[test]
        fn test_sanitize_secret_name_mixed_invalid_chars() {
            let result = sanitize_secret_name("my.service/database url");
            assert_eq!(result, "my_service_database_url");
        }

        #[test]
        fn test_sanitize_secret_name_valid_chars() {
            let result = sanitize_secret_name("my-service_database-url123");
            assert_eq!(result, "my-service_database-url123");
        }

        #[test]
        fn test_sanitize_secret_name_special_chars() {
            let result = sanitize_secret_name("my@service#database$url");
            assert_eq!(result, "my_service_database_url");
        }

        #[test]
        fn test_construct_secret_name_with_sanitization() {
            // Test that construct_secret_name applies sanitization
            let result = construct_secret_name(Some("my.service"), "database/url", Some("-prod"));
            assert_eq!(result, "my_service-database_url-prod"); // Leading dash stripped, invalid chars sanitized
        }

        #[test]
        fn test_construct_secret_name_kustomize_compatibility() {
            // Test compatibility with kustomize-google-secret-manager naming
            let result = construct_secret_name(Some("idam-dev"), "database-url", Some("-prod"));
            assert_eq!(result, "idam-dev-database-url-prod"); // Leading dash stripped from suffix
        }

        #[test]
        fn test_construct_secret_name_edge_cases() {
            // Test edge cases
            assert_eq!(construct_secret_name(None, "", None), "");
            assert_eq!(
                construct_secret_name(Some("prefix"), "", Some("suffix")),
                "prefix-suffix"
            ); // Empty key becomes empty string after trim
            assert_eq!(construct_secret_name(Some("a"), "b", Some("c")), "a-b-c");
            assert_eq!(
                construct_secret_name(Some("prefix"), "key", Some("---suffix")),
                "prefix-key-suffix"
            ); // Multiple leading dashes stripped
        }

        #[test]
        fn test_sanitize_secret_name_edge_cases() {
            // Test edge cases
            assert_eq!(sanitize_secret_name(""), "");
            assert_eq!(sanitize_secret_name("a"), "a");
            assert_eq!(sanitize_secret_name("___"), "___");
            assert_eq!(sanitize_secret_name("---"), ""); // All dashes removed by trim
            assert_eq!(sanitize_secret_name("--test--"), "test"); // Leading/trailing dashes removed
            assert_eq!(sanitize_secret_name("a--b--c"), "a-b-c"); // Consecutive dashes collapsed
        }
    }
}
