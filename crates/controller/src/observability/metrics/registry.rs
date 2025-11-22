//! # Metrics Registry
//!
//! Prometheus metrics registry setup and registration.

use anyhow::Result;
use prometheus::Registry;
use std::sync::LazyLock;

/// Global Prometheus metrics registry
pub(crate) static REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::new);

/// Register all metrics with the Prometheus registry
///
/// This function registers all metrics from all sub-modules.
/// Prometheus Registry::register() takes ownership (Box<dyn Collector>),
/// so we clone the metrics. Since Prometheus metrics internally use Arc,
/// cloning is cheap (just increments a reference count).
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub fn register_metrics() -> Result<()> {
    // Register controller metrics
    super::controller_metrics::register_controller_metrics()?;

    // Register provider metrics
    super::provider_metrics::register_provider_metrics()?;

    // Register processing metrics
    super::processing_metrics::register_processing_metrics()?;

    Ok(())
}
