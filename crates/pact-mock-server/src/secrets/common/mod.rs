//! Common secret store implementation
//!
//! Provides the core in-memory secret store with versioning support.
//! This is shared across all provider-specific implementations.

pub mod errors;
pub mod limits;
pub mod store;

// Re-export for convenience
pub use store::{SecretEntry, SecretStore, SecretVersion};
