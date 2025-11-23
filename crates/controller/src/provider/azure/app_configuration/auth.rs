//! # Azure App Configuration Authentication
//!
//! Handles authentication for Azure App Configuration, including token management.

use crate::crd::{AzureAuthConfig, AzureConfig};
use anyhow::{Context, Result};
use azure_core::credentials::{AccessToken, TokenCredential, TokenRequestOptions};
use azure_identity::{ManagedIdentityCredential, WorkloadIdentityCredential};
use std::sync::Arc;
use tracing::info;

/// Create Azure App Configuration credential based on configuration
/// Supports Workload Identity, Managed Identity, and mock credentials for Pact testing
pub fn create_credential(config: &AzureConfig) -> Result<Arc<dyn TokenCredential>> {
    let credential: Arc<dyn TokenCredential> = if std::env::var("PACT_MODE").is_ok() {
        // Use mock credential for Pact testing
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
                WorkloadIdentityCredential::new(Some(options))
                    .context("Failed to create WorkloadIdentityCredential")?
            }
            None => {
                info!("No auth configuration specified, using Managed Identity");
                info!("This works automatically in Azure environments (AKS, App Service, etc.)");
                ManagedIdentityCredential::new(None)
                    .context("Failed to create ManagedIdentityCredential")?
            }
        }
    };

    Ok(credential)
}

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
        use typespec_client_core::time::{Duration, OffsetDateTime};

        Ok(AccessToken::new(
            azure_core::credentials::Secret::new("test-token".to_string()),
            OffsetDateTime::now_utc() + Duration::seconds(3600),
        ))
    }
}

/// Get access token for Azure App Configuration
pub async fn get_token(credential: &Arc<dyn TokenCredential>) -> Result<String> {
    let scope = &["https://appconfig.azure.net/.default"];
    let options = Some(TokenRequestOptions::default());
    let token_response = credential
        .get_token(scope, options)
        .await
        .context("Failed to get Azure App Configuration access token")?;
    Ok(token_response.token.secret().to_string())
}
