//! # Status Management
//!
//! Updates SecretManagerConfig status with reconciliation results.

use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::validation::parse_kubernetes_duration;
use crate::{Condition, SecretManagerConfig, SecretManagerConfigStatus};
use anyhow::Result;
use kube::api::PatchParams;
use tracing::{debug, warn};

/// Update status phase and description
/// CRITICAL: Checks if status actually changed before updating to prevent unnecessary watch events
pub async fn update_status_phase(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
    phase: &str,
    message: Option<&str>,
) -> Result<()> {
    // CRITICAL: Check if status actually changed before updating
    // This prevents unnecessary status updates that trigger watch events
    let current_phase = config.status.as_ref().and_then(|s| s.phase.as_deref());
    let current_description = config
        .status
        .as_ref()
        .and_then(|s| s.description.as_deref());

    // Only update if phase or description actually changed
    if current_phase == Some(phase) && current_description == message.as_deref() {
        debug!(
            "Skipping status update - phase and description unchanged: phase={:?}, description={:?}",
            phase, message
        );
        return Ok(());
    }

    let api: kube::Api<SecretManagerConfig> = kube::Api::namespaced(
        reconciler.client.clone(),
        config.metadata.namespace.as_deref().unwrap_or("default"),
    );

    let mut conditions = vec![];
    let ready_status = if phase == "Ready" { "True" } else { "False" };
    let ready_reason = if phase == "Ready" {
        "ReconciliationSucceeded"
    } else if phase == "Failed" {
        "ReconciliationFailed"
    } else {
        "ReconciliationInProgress"
    };

    conditions.push(Condition {
        r#type: "Ready".to_string(),
        status: ready_status.to_string(),
        last_transition_time: Some(chrono::Utc::now().to_rfc3339()),
        reason: Some(ready_reason.to_string()),
        message: message.map(|s| s.to_string()),
    });

    // Calculate next reconcile time based on reconcile interval
    let next_reconcile_time = parse_kubernetes_duration(&config.spec.reconcile_interval)
        .ok()
        .map(|duration| {
            chrono::Utc::now()
                .checked_add_signed(
                    chrono::Duration::from_std(duration).unwrap_or(chrono::Duration::zero()),
                )
                .map(|dt| dt.to_rfc3339())
        })
        .flatten();

    let status = SecretManagerConfigStatus {
        phase: Some(phase.to_string()),
        description: message.map(|s| s.to_string()),
        conditions,
        observed_generation: config.metadata.generation,
        last_reconcile_time: Some(chrono::Utc::now().to_rfc3339()),
        next_reconcile_time,
        secrets_synced: None,
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

/// Update status with secrets synced count
/// CRITICAL: Checks if status actually changed before updating to prevent unnecessary watch events
pub async fn update_status(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
    secrets_synced: i32,
) -> Result<()> {
    use kube::api::PatchParams;

    // Determine what was synced for the description
    let is_configs_enabled = config
        .spec
        .configs
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(false);
    let description = if is_configs_enabled {
        format!("Synced {secrets_synced} properties to config store")
    } else {
        format!("Synced {secrets_synced} secrets to secret store")
    };

    // CRITICAL: Check if status actually changed before updating
    // This prevents unnecessary status updates that trigger watch events
    let current_phase = config.status.as_ref().and_then(|s| s.phase.as_deref());
    let current_secrets_synced = config.status.as_ref().and_then(|s| s.secrets_synced);

    // Only update if phase changed (not Ready) or secrets_synced count changed
    if current_phase == Some("Ready") && current_secrets_synced == Some(secrets_synced) {
        debug!(
            "Skipping status update - already Ready with same secrets_synced count: {}",
            secrets_synced
        );
        return Ok(());
    }

    let api: kube::Api<SecretManagerConfig> = kube::Api::namespaced(
        reconciler.client.clone(),
        config.metadata.namespace.as_deref().unwrap_or("default"),
    );

    let status = SecretManagerConfigStatus {
        phase: Some("Ready".to_string()),
        description: Some(description.clone()),
        conditions: vec![Condition {
            r#type: "Ready".to_string(),
            status: "True".to_string(),
            last_transition_time: Some(chrono::Utc::now().to_rfc3339()),
            reason: Some("ReconciliationSucceeded".to_string()),
            message: Some(description),
        }],
        observed_generation: config.metadata.generation,
        last_reconcile_time: Some(chrono::Utc::now().to_rfc3339()),
        next_reconcile_time: parse_kubernetes_duration(&config.spec.reconcile_interval)
            .ok()
            .map(|duration| {
                chrono::Utc::now()
                    .checked_add_signed(
                        chrono::Duration::from_std(duration).unwrap_or(chrono::Duration::zero()),
                    )
                    .map(|dt| dt.to_rfc3339())
            })
            .flatten(),
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

/// Calculate progressive backoff duration based on error count using Fibonacci sequence
/// Fibonacci backoff: 1m -> 1m -> 2m -> 3m -> 5m -> 8m -> 13m -> 21m -> 34m -> 55m -> 60m (1 hour max)
/// This prevents controller overload when parsing errors occur
/// Each resource maintains its own error count independently
pub fn calculate_progressive_backoff(error_count: u32) -> std::time::Duration {
    // Fibonacci sequence for backoff (in minutes): 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, then cap at 60
    // This provides exponential growth that naturally slows down as errors accumulate
    let backoff_minutes = match error_count {
        0 => 1,  // First error: 1 minute
        1 => 1,  // Second error: 1 minute
        2 => 2,  // Third error: 2 minutes
        3 => 3,  // Fourth error: 3 minutes
        4 => 5,  // Fifth error: 5 minutes
        5 => 8,  // Sixth error: 8 minutes
        6 => 13, // Seventh error: 13 minutes
        7 => 21, // Eighth error: 21 minutes
        8 => 34, // Ninth error: 34 minutes
        9 => 55, // Tenth error: 55 minutes
        _ => 60, // Eleventh+ error: 60 minutes (1 hour max)
    };

    std::time::Duration::from_secs(backoff_minutes * 60)
}

/// Get parsing error count from resource annotations
/// Each resource maintains its own error count independently
/// Returns the current error count for THIS resource or 0 if not set
pub fn get_parsing_error_count(config: &SecretManagerConfig) -> u32 {
    // Each resource has its own annotations, so error counts are per-resource
    config
        .metadata
        .annotations
        .as_ref()
        .and_then(|ann| {
            ann.get("secret-management.microscaler.io/duration-parsing-errors")
                .and_then(|v| v.parse::<u32>().ok())
        })
        .unwrap_or(0)
}

/// Increment parsing error count in resource annotations
/// Each resource maintains its own error count independently
/// This persists the error count across reconciliations and controller restarts
pub async fn increment_parsing_error_count(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
    current_count: u32,
) -> Result<()> {
    use kube::api::PatchParams;

    // Each resource is patched individually, so error counts are per-resource
    let api: kube::Api<SecretManagerConfig> = kube::Api::namespaced(
        reconciler.client.clone(),
        config.metadata.namespace.as_deref().unwrap_or("default"),
    );

    let new_count = current_count + 1;
    let patch = serde_json::json!({
        "metadata": {
            "annotations": {
                "secret-management.microscaler.io/duration-parsing-errors": new_count.to_string()
            }
        }
    });

    // Patch THIS specific resource's annotations
    // Other resources are unaffected
    api.patch(
        config.metadata.name.as_deref().unwrap_or("unknown"),
        &PatchParams::apply("secret-manager-controller"),
        &kube::api::Patch::Merge(patch),
    )
    .await?;

    Ok(())
}

/// Clear parsing error count from resource annotations
/// Called when parsing succeeds to reset the backoff for THIS resource
/// Each resource's error count is cleared independently
pub async fn clear_parsing_error_count(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
) -> Result<()> {
    use kube::api::PatchParams;

    // Each resource is patched individually, so clearing is per-resource
    let api: kube::Api<SecretManagerConfig> = kube::Api::namespaced(
        reconciler.client.clone(),
        config.metadata.namespace.as_deref().unwrap_or("default"),
    );

    // Only clear if annotation exists for THIS resource
    if let Some(ann) = &config.metadata.annotations {
        if ann.contains_key("secret-management.microscaler.io/duration-parsing-errors") {
            let patch = serde_json::json!({
                "metadata": {
                    "annotations": {
                        "secret-management.microscaler.io/duration-parsing-errors": null
                    }
                }
            });

            // Clear annotation for THIS specific resource only
            // Other resources' error counts remain unchanged
            api.patch(
                config.metadata.name.as_deref().unwrap_or("unknown"),
                &PatchParams::apply("secret-manager-controller"),
                &kube::api::Patch::Merge(patch),
            )
            .await?;
        }
    }

    Ok(())
}

/// Clear manual trigger annotation after reconciliation completes
/// This prevents the annotation from triggering repeated reconciliations
/// Called after successful reconciliation when manual trigger was detected
pub async fn clear_manual_trigger_annotation(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
) -> Result<()> {
    use kube::api::PatchParams;

    let api: kube::Api<SecretManagerConfig> = kube::Api::namespaced(
        reconciler.client.clone(),
        config.metadata.namespace.as_deref().unwrap_or("default"),
    );

    // Only clear if annotation exists
    if let Some(ann) = &config.metadata.annotations {
        if ann.contains_key("secret-management.microscaler.io/reconcile") {
            let patch = serde_json::json!({
                "metadata": {
                    "annotations": {
                        "secret-management.microscaler.io/reconcile": null
                    }
                }
            });

            api.patch(
                config.metadata.name.as_deref().unwrap_or("unknown"),
                &PatchParams::apply("secret-manager-controller"),
                &kube::api::Patch::Merge(patch),
            )
            .await?;
        }
    }

    Ok(())
}
