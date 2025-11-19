//! Controller Edge Cases Integration Tests
//!
//! Tests for edge cases in the controller's reconciliation flow:
//! - GitRepository not found (404)
//! - GitRepository not ready
//! - SOPS decryption failures
//! - Partial failures
//! - Secret deletion and re-creation
//! - Concurrent version creation
//! - Invalid secret names
//! - Kustomize build failures
//! - Artifact download failures
//! - Version-specific operations
//! - Version disabling
//! - Version deletion and recreation
//! - Multiple services in same repo

pub mod gitrepository;
pub mod sops;
pub mod partial_failures;
pub mod secret_operations;
pub mod artifact_failures;
pub mod version_operations;

