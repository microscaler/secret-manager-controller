//! # Azure Key Vault Authentication
//!
//! Handles authentication for Azure Key Vault, including mock credentials for Pact testing.

use crate::crd::{AzureAuthConfig, AzureConfig};
use anyhow::{Context, Result};
use azure_core::credentials::{AccessToken, Secret, TokenCredential, TokenRequestOptions};
use azure_identity::{ManagedIdentityCredential, WorkloadIdentityCredential};
use std::sync::Arc;
use tracing::{debug, info};

/// Mock TokenCredential for Pact testing
/// Returns a dummy token without attempting real Azure authentication
#[derive(Debug)]
pub struct MockTokenCredential;

#[async_trait::async_trait]
impl TokenCredential for MockTokenCredential {
    async fn get_token(
        &self,
        _scopes: &[&str],
        _options: Option<TokenRequestOptions<'_>>,
    ) -> azure_core::Result<AccessToken> {
        // Return a mock access token for Pact testing
        // The trait returns AccessToken, which has a .token field of type Secret
        // AccessToken::new(token: Secret, expires_on: OffsetDateTime)
        use typespec_client_core::time::{Duration, OffsetDateTime};

        Ok(AccessToken::new(
            Secret::new("test-token".to_string()),
            OffsetDateTime::now_utc() + Duration::seconds(3600),
        ))
    }
}

/// Create Azure credential based on configuration
/// Supports Workload Identity, Managed Identity, and mock credentials for Pact testing
pub fn create_credential(config: &AzureConfig) -> Result<Arc<dyn TokenCredential>> {
    // In Pact mode, use a mock credential that returns a dummy token
    let credential: Arc<dyn TokenCredential> = if std::env::var("PACT_MODE").is_ok() {
        // Use mock credential for Pact tests
        debug!("Pact mode: using mock Azure credential");
        Arc::new(MockTokenCredential)
    } else {
        match &config.auth {
            Some(AzureAuthConfig::WorkloadIdentity { client_id }) => {
                info!(
                    "Using Azure Workload Identity authentication with client ID: {}",
                    client_id
                );
                info!("Ensure pod service account has Azure Workload Identity configured");
                let options = azure_identity::WorkloadIdentityCredentialOptions {
                    client_id: Some(client_id.clone()),
                    ..Default::default()
                };
                // Note: Credential constructors return Arc<dyn TokenCredential>
                WorkloadIdentityCredential::new(Some(options))
                    .context("Failed to create WorkloadIdentityCredential")?
            }
            None => {
                // Default to Managed Identity (works in Azure environments like AKS)
                info!("No auth configuration specified, using Managed Identity");
                info!("This works automatically in Azure environments (AKS, App Service, etc.)");
                // Note: Credential constructors return Arc<dyn TokenCredential>
                ManagedIdentityCredential::new(None)
                    .context("Failed to create ManagedIdentityCredential")?
            }
        }
    };

    Ok(credential)
}
