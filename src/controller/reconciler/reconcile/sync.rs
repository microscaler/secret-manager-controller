//! # Secret Syncing
//!
//! Handles syncing secrets from artifact path to cloud provider.

use crate::controller::parser;
use crate::controller::reconciler::processing::{
    process_application_files, process_kustomize_secrets,
};
use crate::controller::reconciler::status::update_status_phase;
use crate::controller::reconciler::types::{Reconciler, ReconcilerError};
use crate::crd::SecretManagerConfig;
use crate::observability;
use crate::provider::SecretManagerProvider;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Result type for secret syncing
#[derive(Debug)]
pub enum SyncResult {
    /// Successfully synced secrets (count)
    Success(u32),
    /// Transient error - should retry
    TransientError,
    /// Permanent error
    Error(ReconcilerError),
}

/// Sync secrets from artifact path to provider
pub async fn sync_secrets(
    config: &Arc<SecretManagerConfig>,
    ctx: &Arc<Reconciler>,
    provider: &dyn SecretManagerProvider,
    artifact_path: &PathBuf,
) -> Result<SyncResult, ReconcilerError> {
    let mut secrets_synced = 0;

    // Determine processing mode: kustomize build vs raw file parsing
    // Kustomize mode: Extract secrets from kustomize-generated Secret resources
    // Raw file mode: Parse application.secrets.env files directly
    if let Some(kustomize_path) = &config.spec.secrets.kustomize_path {
        // Kustomize Build Mode
        // Runs `kustomize build` to generate Kubernetes manifests, then extracts Secret resources
        // Supports overlays, patches, generators, and other kustomize features
        // This is the recommended mode for complex deployments with multiple environments
        info!("Using kustomize build mode on path: {}", kustomize_path);

        match crate::controller::kustomize::extract_secrets_from_kustomize(
            artifact_path,
            kustomize_path,
        ) {
            Ok(secrets) => {
                let secret_prefix = config.spec.secrets.prefix.as_deref().unwrap_or("default");
                match process_kustomize_secrets(provider, config, &secrets, secret_prefix).await {
                    Ok(count) => {
                        secrets_synced += count as u32;
                        info!("✅ Synced {} secrets from kustomize build", count);
                    }
                    Err(e) => {
                        error!("Failed to process kustomize secrets: {}", e);
                        observability::metrics::increment_reconciliation_errors();
                        // Update status to Failed
                        let _ = update_status_phase(
                            ctx,
                            config,
                            "Failed",
                            Some(&format!("Failed to process kustomize secrets: {e}")),
                        )
                        .await;
                        return Ok(SyncResult::Error(ReconcilerError::ReconciliationFailed(e)));
                    }
                }
            }
            Err(e) => {
                error!("Failed to extract secrets from kustomize build: {}", e);
                observability::metrics::increment_reconciliation_errors();
                // Update status to Failed
                let _ = update_status_phase(
                    ctx,
                    config,
                    "Failed",
                    Some(&format!("Failed to extract secrets from kustomize: {e}")),
                )
                .await;
                return Ok(SyncResult::Error(ReconcilerError::ReconciliationFailed(e)));
            }
        }
    } else {
        // Raw File Mode
        // Directly parses application.secrets.env, application.secrets.yaml, and application.properties files
        // Simpler than kustomize mode but doesn't support overlays or generators
        // Suitable for simple deployments or when kustomize isn't needed
        info!("Using raw file mode");

        // Find application files for the specified environment
        // Searches for files matching patterns like:
        // - {basePath}/profiles/{environment}/application.secrets.env
        // - {basePath}/{service}/profiles/{environment}/application.secrets.env
        // Pass secret_prefix as default_service_name for single service deployments
        let default_service_name = config.spec.secrets.prefix.as_deref();
        let application_files = match parser::find_application_files(
            artifact_path,
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
                // Update status to Failed
                let _ = update_status_phase(
                    ctx,
                    config,
                    "Failed",
                    Some(&format!("Failed to find application files: {e}")),
                )
                .await;
                return Ok(SyncResult::Error(ReconcilerError::ReconciliationFailed(e)));
            }
        };

        info!(
            "📋 Found {} application file set(s) to process",
            application_files.len()
        );

        // Process each application file set
        for app_files in application_files {
            match process_application_files(ctx, provider, config, &app_files).await {
                Ok(count) => {
                    secrets_synced += count as u32;
                    info!(
                        "✅ Synced {} secrets for service: {}",
                        count, app_files.service_name
                    );
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    // Check if this is a transient SOPS decryption error
                    let is_transient = error_msg.contains("transient");

                    if is_transient {
                        // Transient error - log warning and return action to retry
                        warn!(
                            "⏳ Transient error processing service {}: {}. Will retry.",
                            app_files.service_name, error_msg
                        );
                        observability::metrics::increment_reconciliation_errors();
                        // Update status to indicate retry
                        let _ = update_status_phase(
                            ctx,
                            config,
                            "Retrying",
                            Some(&format!("Transient error: {}. Retrying...", error_msg)),
                        )
                        .await;
                        // Return action to retry after a delay
                        return Ok(SyncResult::TransientError);
                    } else {
                        // Permanent error - log error and continue with other services
                        // This allows partial success when multiple services are configured
                        error!(
                            "❌ Permanent error processing service {}: {}",
                            app_files.service_name, error_msg
                        );
                        observability::metrics::increment_reconciliation_errors();
                        // Update status to indicate failure for this service
                        let _ = update_status_phase(
                            ctx,
                            config,
                            "PartialFailure",
                            Some(&format!(
                                "Failed to process service {}: {}",
                                app_files.service_name, error_msg
                            )),
                        )
                        .await;
                    }
                }
            }
        }
    }

    Ok(SyncResult::Success(secrets_synced))
}
