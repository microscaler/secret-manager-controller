//! Secret Manager Controller Library
//!
//! This library provides the core functionality for the Secret Manager Controller.
//! Tests are included in the module files (e.g., reconciler.rs).

// Re-export modules so they can be tested
pub mod config;
pub mod constants;
pub mod controller;
pub mod crd;
pub mod observability;
pub mod provider;
pub mod runtime;

// Re-export CRD types for convenience
pub use crd::*;
