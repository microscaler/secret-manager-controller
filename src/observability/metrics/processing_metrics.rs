//! # Processing Metrics
//!
//! Metrics for processing operations: SOPS decryption, Kustomize builds, Git operations,
//! artifact handling, and duration parsing.

use crate::observability::metrics::registry::REGISTRY;
use anyhow::Result;
use prometheus::{Histogram, IntCounter, IntCounterVec};
use std::sync::LazyLock;

// Duration parsing errors
static DURATION_PARSING_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_duration_parsing_errors_total",
        "Total number of duration parsing errors (reconcileInterval parsing failures)",
    )
    .expect("Failed to create DURATION_PARSING_ERRORS_TOTAL metric - this should never happen")
});

// SOPS decryption metrics
static SOPS_DECRYPTION_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_sops_decryption_total",
        "Total number of SOPS decryption operations (attempts)",
    )
    .expect("Failed to create SOPS_DECRYPTION_TOTAL metric - this should never happen")
});

static SOPS_DECRYPTION_SUCCESS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_sops_decrypt_success_total",
        "Total number of successful SOPS decryption operations",
    )
    .expect("Failed to create SOPS_DECRYPTION_SUCCESS_TOTAL metric - this should never happen")
});

static SOPS_DECRYPTION_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_sops_decrypt_duration_seconds",
            "Duration of SOPS decryption operations in seconds",
        )
        .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0]),
    )
    .expect("Failed to create SOPS_DECRYPTION_DURATION metric - this should never happen")
});

static SOPS_DECRYPTION_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_sops_decryption_errors_total",
        "Total number of SOPS decryption errors",
    )
    .expect("Failed to create SOPS_DECRYPTION_ERRORS_TOTAL metric - this should never happen")
});

static SOPS_DECRYPTION_ERRORS_TOTAL_BY_REASON: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        prometheus::Opts::new(
            "secret_manager_sops_decryption_errors_total_by_reason",
            "Total number of SOPS decryption errors by failure reason",
        ),
        &["reason"],
    )
    .expect(
        "Failed to create SOPS_DECRYPTION_ERRORS_TOTAL_BY_REASON metric - this should never happen",
    )
});

// Kustomize build metrics
static KUSTOMIZE_BUILD_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_kustomize_build_total",
        "Total number of kustomize build operations",
    )
    .expect("Failed to create KUSTOMIZE_BUILD_TOTAL metric - this should never happen")
});

static KUSTOMIZE_BUILD_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_kustomize_build_duration_seconds",
            "Duration of kustomize build operations in seconds",
        )
        .buckets(vec![0.5, 1.0, 2.0, 5.0, 10.0, 30.0]),
    )
    .expect("Failed to create KUSTOMIZE_BUILD_DURATION metric - this should never happen")
});

static KUSTOMIZE_BUILD_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_kustomize_build_errors_total",
        "Total number of kustomize build errors",
    )
    .expect("Failed to create KUSTOMIZE_BUILD_ERRORS_TOTAL metric - this should never happen")
});

// Git clone metrics
static GIT_CLONE_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_git_clone_total",
        "Total number of git clone operations",
    )
    .expect("Failed to create GIT_CLONE_TOTAL metric - this should never happen")
});

static GIT_CLONE_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_git_clone_duration_seconds",
            "Duration of git clone operations in seconds",
        )
        .buckets(vec![1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
    )
    .expect("Failed to create GIT_CLONE_DURATION metric - this should never happen")
});

static GIT_CLONE_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_git_clone_errors_total",
        "Total number of git clone errors",
    )
    .expect("Failed to create GIT_CLONE_ERRORS_TOTAL metric - this should never happen")
});

// Artifact download and extraction metrics
static ARTIFACT_DOWNLOADS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_artifact_downloads_total",
        "Total number of artifact downloads (FluxCD/ArgoCD)",
    )
    .expect("Failed to create ARTIFACT_DOWNLOADS_TOTAL metric - this should never happen")
});

static ARTIFACT_DOWNLOAD_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_artifact_download_duration_seconds",
            "Duration of artifact downloads in seconds",
        )
        .buckets(vec![0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
    )
    .expect("Failed to create ARTIFACT_DOWNLOAD_DURATION metric - this should never happen")
});

static ARTIFACT_DOWNLOAD_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_artifact_download_errors_total",
        "Total number of artifact download errors",
    )
    .expect("Failed to create ARTIFACT_DOWNLOAD_ERRORS_TOTAL metric - this should never happen")
});

static ARTIFACT_EXTRACTIONS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_artifact_extractions_total",
        "Total number of artifact extractions",
    )
    .expect("Failed to create ARTIFACT_EXTRACTIONS_TOTAL metric - this should never happen")
});

static ARTIFACT_EXTRACTION_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_artifact_extraction_duration_seconds",
            "Duration of artifact extractions in seconds",
        )
        .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0]),
    )
    .expect("Failed to create ARTIFACT_EXTRACTION_DURATION metric - this should never happen")
});

static ARTIFACT_EXTRACTION_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_artifact_extraction_errors_total",
        "Total number of artifact extraction errors",
    )
    .expect("Failed to create ARTIFACT_EXTRACTION_ERRORS_TOTAL metric - this should never happen")
});

/// Register processing metrics with the registry
pub(crate) fn register_processing_metrics() -> Result<()> {
    REGISTRY.register(Box::new(DURATION_PARSING_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SOPS_DECRYPTION_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SOPS_DECRYPTION_SUCCESS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SOPS_DECRYPTION_DURATION.clone()))?;
    REGISTRY.register(Box::new(SOPS_DECRYPTION_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SOPS_DECRYPTION_ERRORS_TOTAL_BY_REASON.clone()))?;
    REGISTRY.register(Box::new(KUSTOMIZE_BUILD_TOTAL.clone()))?;
    REGISTRY.register(Box::new(KUSTOMIZE_BUILD_DURATION.clone()))?;
    REGISTRY.register(Box::new(KUSTOMIZE_BUILD_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(GIT_CLONE_TOTAL.clone()))?;
    REGISTRY.register(Box::new(GIT_CLONE_DURATION.clone()))?;
    REGISTRY.register(Box::new(GIT_CLONE_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_DOWNLOADS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_DOWNLOAD_DURATION.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_DOWNLOAD_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_EXTRACTIONS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_EXTRACTION_DURATION.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_EXTRACTION_ERRORS_TOTAL.clone()))?;
    Ok(())
}

// Public functions for processing metrics

pub fn increment_duration_parsing_errors() {
    DURATION_PARSING_ERRORS_TOTAL.inc();
}

pub fn increment_sops_decryption_total() {
    SOPS_DECRYPTION_TOTAL.inc();
}

pub fn increment_sops_decrypt_success_total() {
    SOPS_DECRYPTION_SUCCESS_TOTAL.inc();
}

pub fn observe_sops_decryption_duration(duration: f64) {
    SOPS_DECRYPTION_DURATION.observe(duration);
}

pub fn increment_sops_decryption_errors_total() {
    SOPS_DECRYPTION_ERRORS_TOTAL.inc();
}

/// Increment SOPS decryption errors counter with reason label
pub fn increment_sops_decryption_errors_total_with_reason(reason: &str) {
    SOPS_DECRYPTION_ERRORS_TOTAL.inc();
    SOPS_DECRYPTION_ERRORS_TOTAL_BY_REASON
        .with_label_values(&[reason])
        .inc();
}

pub fn increment_kustomize_build_total() {
    KUSTOMIZE_BUILD_TOTAL.inc();
}

pub fn observe_kustomize_build_duration(duration: f64) {
    KUSTOMIZE_BUILD_DURATION.observe(duration);
}

pub fn increment_kustomize_build_errors_total() {
    KUSTOMIZE_BUILD_ERRORS_TOTAL.inc();
}

pub fn increment_git_clone_total() {
    GIT_CLONE_TOTAL.inc();
}

pub fn observe_git_clone_duration(duration: f64) {
    GIT_CLONE_DURATION.observe(duration);
}

pub fn increment_git_clone_errors_total() {
    GIT_CLONE_ERRORS_TOTAL.inc();
}

pub fn increment_artifact_downloads_total() {
    ARTIFACT_DOWNLOADS_TOTAL.inc();
}

pub fn observe_artifact_download_duration(duration: f64) {
    ARTIFACT_DOWNLOAD_DURATION.observe(duration);
}

pub fn increment_artifact_download_errors_total() {
    ARTIFACT_DOWNLOAD_ERRORS_TOTAL.inc();
}

pub fn increment_artifact_extractions_total() {
    ARTIFACT_EXTRACTIONS_TOTAL.inc();
}

pub fn observe_artifact_extraction_duration(duration: f64) {
    ARTIFACT_EXTRACTION_DURATION.observe(duration);
}

pub fn increment_artifact_extraction_errors_total() {
    ARTIFACT_EXTRACTION_ERRORS_TOTAL.inc();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increment_duration_parsing_errors() {
        let before = DURATION_PARSING_ERRORS_TOTAL.get();
        increment_duration_parsing_errors();
        let after = DURATION_PARSING_ERRORS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_increment_sops_decryption_total() {
        let before = SOPS_DECRYPTION_TOTAL.get();
        increment_sops_decryption_total();
        let after = SOPS_DECRYPTION_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_increment_sops_decrypt_success_total() {
        let before = SOPS_DECRYPTION_SUCCESS_TOTAL.get();
        increment_sops_decrypt_success_total();
        let after = SOPS_DECRYPTION_SUCCESS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_observe_sops_decryption_duration() {
        observe_sops_decryption_duration(0.2);
        // Just verify it doesn't panic
    }

    #[test]
    fn test_increment_sops_decryption_errors_total() {
        let before = SOPS_DECRYPTION_ERRORS_TOTAL.get();
        increment_sops_decryption_errors_total();
        let after = SOPS_DECRYPTION_ERRORS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_increment_kustomize_build_total() {
        let before = KUSTOMIZE_BUILD_TOTAL.get();
        increment_kustomize_build_total();
        let after = KUSTOMIZE_BUILD_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_observe_kustomize_build_duration() {
        observe_kustomize_build_duration(1.0);
        // Just verify it doesn't panic
    }

    #[test]
    fn test_increment_kustomize_build_errors_total() {
        let before = KUSTOMIZE_BUILD_ERRORS_TOTAL.get();
        increment_kustomize_build_errors_total();
        let after = KUSTOMIZE_BUILD_ERRORS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_increment_git_clone_total() {
        let before = GIT_CLONE_TOTAL.get();
        increment_git_clone_total();
        let after = GIT_CLONE_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_observe_git_clone_duration() {
        observe_git_clone_duration(2.5);
        // Just verify it doesn't panic
    }

    #[test]
    fn test_increment_git_clone_errors_total() {
        let before = GIT_CLONE_ERRORS_TOTAL.get();
        increment_git_clone_errors_total();
        let after = GIT_CLONE_ERRORS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }
}
