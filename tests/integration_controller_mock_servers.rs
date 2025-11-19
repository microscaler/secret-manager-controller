//! Controller Integration Tests with Mock Servers
//!
//! These tests verify the full controller reconciliation flow by:
//! 1. Starting mock servers (GCP, AWS, Azure)
//! 2. Creating SecretManagerConfig resources
//! 3. Triggering reconciliation
//! 4. Verifying secrets are created/updated in mock servers
//! 5. Testing versioning behavior
//! 6. Testing error handling
//!
//! **Note**: These tests require:
//! - Mock server binaries to be built
//! - A Kubernetes cluster (or use `kube-test` for in-memory testing)
//! - Tests should run sequentially to avoid port conflicts
//!
//! Run with: `cargo test --test integration_controller_mock_servers -- --test-threads=1`

#[path = "integration/controller_mock_servers/mod.rs"]
mod controller_mock_servers;

pub use controller_mock_servers::*;
