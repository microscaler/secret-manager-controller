//! # Controller Metrics
//!
//! Metrics for controller operations: reconciliations, secrets management, and requeues.

use crate::observability::metrics::registry::REGISTRY;
use anyhow::Result;
use prometheus::{Histogram, IntCounter, IntCounterVec, IntGauge};
use std::sync::LazyLock;

// Controller reconciliation metrics
static RECONCILIATIONS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_reconciliations_total",
        "Total number of reconciliations",
    )
    .expect("Failed to create RECONCILIATIONS_TOTAL metric - this should never happen")
});

static RECONCILIATION_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_reconciliation_errors_total",
        "Total number of reconciliation errors",
    )
    .expect("Failed to create RECONCILIATION_ERRORS_TOTAL metric - this should never happen")
});

static RECONCILIATION_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_reconciliation_duration_seconds",
            "Duration of reconciliation in seconds",
        )
        .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]),
    )
    .expect("Failed to create RECONCILIATION_DURATION metric - this should never happen")
});

// Secrets management metrics
static SECRETS_SYNCED_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_secrets_synced_total",
        "Total number of secrets synced to GCP Secret Manager",
    )
    .expect("Failed to create SECRETS_SYNCED_TOTAL metric - this should never happen")
});

static SECRETS_UPDATED_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_secrets_updated_total",
        "Total number of secrets updated (overwritten from git)",
    )
    .expect("Failed to create SECRETS_UPDATED_TOTAL metric - this should never happen")
});

static SECRETS_MANAGED: LazyLock<IntGauge> = LazyLock::new(|| {
    IntGauge::new(
        "secret_manager_secrets_managed",
        "Current number of secrets being managed",
    )
    .expect("Failed to create SECRETS_MANAGED metric - this should never happen")
});

// Requeue metrics
static REQUEUES_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        prometheus::Opts::new(
            "secret_manager_requeues_total",
            "Total number of reconciliation requeues",
        ),
        &["reason"],
    )
    .expect("Failed to create REQUEUES_TOTAL metric - this should never happen")
});

/// Register controller metrics with the registry
pub(crate) fn register_controller_metrics() -> Result<()> {
    REGISTRY.register(Box::new(RECONCILIATIONS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(RECONCILIATION_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(RECONCILIATION_DURATION.clone()))?;
    REGISTRY.register(Box::new(SECRETS_SYNCED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SECRETS_UPDATED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SECRETS_MANAGED.clone()))?;
    REGISTRY.register(Box::new(REQUEUES_TOTAL.clone()))?;
    Ok(())
}

// Public functions for controller metrics

pub fn increment_reconciliations() {
    RECONCILIATIONS_TOTAL.inc();
}

pub fn increment_reconciliation_errors() {
    RECONCILIATION_ERRORS_TOTAL.inc();
}

pub fn observe_reconciliation_duration(duration: f64) {
    RECONCILIATION_DURATION.observe(duration);
}

pub fn increment_secrets_synced(count: i64) {
    #[allow(clippy::cast_sign_loss, reason = "We ensure non-negative with max(0)")]
    let count_u64 = count.max(0) as u64;
    SECRETS_SYNCED_TOTAL.inc_by(count_u64);
}

pub fn increment_secrets_updated(count: i64) {
    #[allow(clippy::cast_sign_loss, reason = "We ensure non-negative with max(0)")]
    let count_u64 = count.max(0) as u64;
    SECRETS_UPDATED_TOTAL.inc_by(count_u64);
}

pub fn set_secrets_managed(count: i64) {
    SECRETS_MANAGED.set(count);
}

pub fn increment_requeues_total(reason: &str) {
    REQUEUES_TOTAL.with_label_values(&[reason]).inc();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increment_reconciliations() {
        let before = RECONCILIATIONS_TOTAL.get();
        increment_reconciliations();
        let after = RECONCILIATIONS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_increment_reconciliation_errors() {
        let before = RECONCILIATION_ERRORS_TOTAL.get();
        increment_reconciliation_errors();
        let after = RECONCILIATION_ERRORS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_observe_reconciliation_duration() {
        observe_reconciliation_duration(1.5);
        // Just verify it doesn't panic - histogram observation doesn't return a value
    }

    #[test]
    fn test_increment_secrets_synced() {
        let before = SECRETS_SYNCED_TOTAL.get();
        increment_secrets_synced(5);
        let after = SECRETS_SYNCED_TOTAL.get();
        assert_eq!(after, before + 5u64);
    }

    #[test]
    fn test_increment_secrets_synced_negative() {
        let before = SECRETS_SYNCED_TOTAL.get();
        increment_secrets_synced(-5); // Should be clamped to 0
        let after = SECRETS_SYNCED_TOTAL.get();
        assert_eq!(after, before); // No change since negative is clamped
    }

    #[test]
    fn test_increment_secrets_updated() {
        let before = SECRETS_UPDATED_TOTAL.get();
        increment_secrets_updated(3);
        let after = SECRETS_UPDATED_TOTAL.get();
        assert_eq!(after, before + 3u64);
    }

    #[test]
    fn test_set_secrets_managed() {
        set_secrets_managed(10);
        assert_eq!(SECRETS_MANAGED.get(), 10);
        set_secrets_managed(20);
        assert_eq!(SECRETS_MANAGED.get(), 20);
    }
}
