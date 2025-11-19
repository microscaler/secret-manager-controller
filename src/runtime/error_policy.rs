//! # Error Policy
//!
//! Error handling and backoff logic for the controller watch loop.
//! This module handles reconciliation errors and watch stream errors.

use crate::constants;
use crate::controller::backoff::FibonacciBackoff;
use crate::controller::reconciler::{BackoffState, Reconciler, ReconcilerError};
use crate::observability;
use kube_runtime::controller::Action;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Handle reconciliation errors with Fibonacci backoff
///
/// This function calculates backoff based on error count for the specific resource,
/// preventing blocking watch/timer paths when many resources fail.
/// Backoff state is tracked per resource to avoid cross-resource interference.
pub fn handle_reconciliation_error(
    obj: Arc<crate::crd::SecretManagerConfig>,
    error: &ReconcilerError,
    ctx: Arc<Reconciler>,
) -> Action {
    let name = obj.metadata.name.as_deref().unwrap_or("unknown");
    let namespace = obj.metadata.namespace.as_deref().unwrap_or("default");

    // Create error span for reconciliation errors
    let error_span = tracing::span!(
        tracing::Level::ERROR,
        "controller.watch.reconciliation_error",
        resource.name = name,
        resource.namespace = namespace,
        error = %error
    );
    let _error_guard = error_span.enter();

    error!("Reconciliation error for {}: {:?}", name, error);
    observability::metrics::increment_reconciliation_errors();

    // Calculate Fibonacci backoff based on error count for this resource
    // This prevents blocking watch/timer paths when many resources fail
    // Backoff state is tracked per resource to avoid cross-resource interference
    // Moved from reconciler to error_policy() layer to prevent deadlocks
    let resource_key = format!("{}/{}", namespace, name);
    let backoff_seconds = match ctx.backoff_states.lock() {
        Ok(mut states) => {
            let state = states
                .entry(resource_key.clone())
                .or_insert_with(|| BackoffState {
                    backoff: FibonacciBackoff::new(1, 10), // 1 minute min, 10 minutes max
                    error_count: 0,
                });
            state.increment_error();
            let backoff = state.backoff.next_backoff_seconds();
            let error_count = state.error_count;
            (backoff, error_count)
        }
        Err(e) => {
            warn!(
                "Failed to lock backoff_states: {}, using default backoff",
                e
            );
            (constants::DEFAULT_RECONCILIATION_ERROR_REQUEUE_SECS, 0)
        }
    };

    let next_trigger_time =
        chrono::Utc::now() + chrono::Duration::seconds(backoff_seconds.0 as i64);

    info!(
        "🔄 Retrying with Fibonacci backoff: {}s (error count: {}, trigger source: error-backoff)",
        backoff_seconds.0, backoff_seconds.1
    );
    info!(
        "📅 Next retry scheduled: {} (in {}s, trigger source: error-backoff)",
        next_trigger_time.to_rfc3339(),
        backoff_seconds.0
    );

    observability::metrics::increment_requeues_total("error-backoff");
    Action::requeue(std::time::Duration::from_secs(backoff_seconds.0))
}

/// Handle watch stream errors with appropriate classification and backoff
///
/// This function classifies watch errors (401, 410, 429, not found, etc.) and
/// applies appropriate handling strategies including backoff and restart logic.
///
/// Returns `None` to filter out the error (allow restart) or `Some(())` to continue.
pub async fn handle_watch_stream_error(
    error_string: &str,
    backoff: &Arc<std::sync::atomic::AtomicU64>,
    max_backoff_ms: u64,
) -> Option<()> {
    // Handle watch errors with proper classification
    let error_span = tracing::span!(
        tracing::Level::WARN,
        "controller.watch.error",
        error = %error_string
    );
    let _error_guard = error_span.enter();

    // Check for specific error types
    let is_401 = error_string.contains("401")
        || error_string.contains("Unauthorized")
        || error_string.contains("WatchFailed");
    let is_410 = error_string.contains("410")
        || error_string.contains("too old resource version")
        || error_string.contains("Expired")
        || error_string.contains("Gone");
    let is_429 = error_string.contains("429")
        || error_string.contains("storage is (re)initializing")
        || error_string.contains("TooManyRequests");
    let is_not_found = error_string.contains("ObjectNotFound")
        || (error_string.contains("404") && error_string.contains("not found"));

    if is_401 {
        // Authentication error - RBAC may have been revoked or token expired
        error!("❌ Watch authentication failed (401 Unauthorized) - RBAC may have been revoked or token expired");
        error!("🔍 SRE Diagnostics:");
        error!("   1. Verify ClusterRole 'secret-manager-controller' still exists:");
        error!("      kubectl get clusterrole secret-manager-controller");
        error!("   2. Verify ClusterRoleBinding still binds ServiceAccount:");
        error!("      kubectl get clusterrolebinding secret-manager-controller -o yaml");
        error!("   3. Verify ServiceAccount still exists:");
        error!("      kubectl get sa secret-manager-controller -n microscaler-system");
        error!("   4. Check if pod ServiceAccount token is valid:");
        error!("      kubectl get pod -n microscaler-system -l app=secret-manager-controller -o jsonpath='{{{{.spec.serviceAccountName}}}}'");
        error!("   5. Verify RBAC permissions are still active:");
        error!("      kubectl auth can-i list secretmanagerconfigs --as=system:serviceaccount:microscaler-system:secret-manager-controller --all-namespaces");
        error!("   6. If RBAC was recently changed, restart the controller pod:");
        error!("      kubectl delete pod -n microscaler-system -l app=secret-manager-controller");
        warn!(
            "⏳ Waiting {}s before retrying watch (RBAC may need time to propagate)...",
            constants::DEFAULT_WATCH_RESTART_DELAY_SECS
        );
        tokio::time::sleep(std::time::Duration::from_secs(
            constants::DEFAULT_WATCH_RESTART_DELAY_SECS,
        ))
        .await;
        None // Filter out to allow restart
    } else if is_410 {
        // Resource version expired - this is normal during pod restarts
        warn!("Watch resource version expired (410) - this is normal during pod restarts, watch will restart");
        warn!(error_type = "410", "watch.error.resource_version_expired");
        None // Filter out to allow restart
    } else if is_429 {
        // Storage reinitializing - back off and let it restart
        let current_backoff = backoff.load(std::sync::atomic::Ordering::Relaxed);
        warn!(
            "API server storage reinitializing (429), backing off for {}ms before restart...",
            current_backoff
        );
        tokio::time::sleep(std::time::Duration::from_millis(current_backoff)).await;
        // Exponential backoff, max configured value
        let new_backoff = std::cmp::min(current_backoff * 2, max_backoff_ms);
        backoff.store(new_backoff, std::sync::atomic::Ordering::Relaxed);
        None // Filter out to allow restart
    } else if is_not_found {
        // Resource not found - this is normal for deleted resources
        warn!("Resource not found (likely deleted), continuing watch...");
        Some(()) // Continue - this is expected
    } else {
        // Other errors - log but continue
        error!("Controller stream error: {}", error_string);
        // For unknown errors, wait a bit before restarting
        tokio::time::sleep(std::time::Duration::from_secs(
            constants::DEFAULT_WATCH_RESTART_DELAY_SECS,
        ))
        .await;
        None // Filter out to allow restart
    }
}
