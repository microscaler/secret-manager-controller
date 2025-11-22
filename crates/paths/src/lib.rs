//! Shared API path definitions for GCP, AWS, and Azure
//!
//! This crate centralizes all API paths to ensure consistency
//! between the controller and mock server implementations for all providers.
//!
//! ## PathBuilder
//!
//! The `PathBuilder` provides a type-safe, builder-pattern API for constructing
//! API paths with different output formats (routes, HTTP paths, response names, etc.).
//!
//! ## Route Constants
//!
//! Route constants are provided for Axum routes, which require static string literals.
//! These constants are validated against PathBuilder output in tests.

pub mod aws;
pub mod azure;
pub mod gcp;

// Core PathBuilder components
pub mod builder;
pub mod errors;
pub mod formats;
pub mod operations;
pub mod provider;

// Re-export core types for convenience
pub use builder::PathBuilder;
pub use errors::PathBuilderError;
pub use formats::PathFormat;
pub use operations::{AwsOperation, AzureOperation, GcpOperation, Operation};
pub use provider::Provider;
