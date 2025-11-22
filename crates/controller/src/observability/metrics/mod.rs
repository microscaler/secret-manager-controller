//! # Metrics Module
//!
//! Prometheus metrics for monitoring the controller, organized by responsibility.
//!
//! ## Sub-modules
//!
//! - `registry` - Metrics registry setup and registration
//! - `controller_metrics` - Controller-specific metrics (reconciliations, secrets, requeues)
//! - `provider_metrics` - Provider-specific metrics (GCP, generic provider operations)
//! - `processing_metrics` - Processing operation metrics (SOPS, Kustomize, Git, Artifacts)

pub mod controller_metrics;
pub mod processing_metrics;
pub mod provider_metrics;
pub mod registry;

// Re-export all public functions for backward compatibility
pub use controller_metrics::*;
pub use processing_metrics::*;
pub use provider_metrics::*;
pub use registry::*;
