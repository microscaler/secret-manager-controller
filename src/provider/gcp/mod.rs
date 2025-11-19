//! # GCP Secret Manager Client
//!
//! Client for interacting with Google Cloud Secret Manager API.
//!
//! This module provides functionality to:
//! - Create and update secrets in GCP Secret Manager
//! - Retrieve secret values
//! - Manage secret versions
//!
//! Uses a native REST implementation that:
//! - Works directly with Pact HTTP mock servers
//! - Uses reqwest with rustls (no OpenSSL dependencies)
//! - Easier to troubleshoot and maintain
//! - Uses reqwest with rustls (no OpenSSL dependencies)

mod client;
pub use client::SecretManagerREST;

use crate::provider::SecretManagerProvider;
use anyhow::Result;
use tracing::info;

/// Create a GCP Secret Manager provider
///
/// Always uses the REST client implementation to avoid SSL/OpenSSL issues.
///
/// # Arguments
/// - `project_id`: GCP project ID
/// - `auth_type`: Authentication type (currently only WorkloadIdentity is supported)
/// - `service_account_email`: Optional service account email for Workload Identity
///
/// # Returns
/// A boxed `SecretManagerProvider` implementation
pub async fn create_gcp_provider(
    project_id: String,
    auth_type: Option<&str>,
    service_account_email: Option<&str>,
) -> Result<Box<dyn SecretManagerProvider>> {
    info!("Using GCP REST client (native implementation)");
    Ok(Box::new(
        SecretManagerREST::new(project_id, auth_type, service_account_email).await?,
    ))
}
