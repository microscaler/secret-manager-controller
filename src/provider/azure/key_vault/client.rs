//! # Azure Key Vault Client Creation
//!
//! Handles creation and initialization of Azure Key Vault client.

use crate::crd::AzureConfig;
use anyhow::{Context, Result};
use azure_core::credentials::TokenCredential;
use azure_security_keyvault_secrets::SecretClient;
use reqwest::Client as ReqwestClient;
use std::sync::Arc;
use tracing::info;

use super::auth::create_credential;

/// Construct vault URL from vault name
/// Supports both full URLs and vault names
/// In Pact mode, uses the endpoint from environment variable
pub fn construct_vault_url(config: &AzureConfig) -> String {
    if std::env::var("PACT_MODE").is_ok() {
        // When PACT_MODE=true, use Pact mock server endpoint
        if let Ok(endpoint) = std::env::var("AZURE_KEY_VAULT_ENDPOINT") {
            info!(
                "Pact mode enabled: routing Azure Key Vault requests to {}",
                endpoint
            );
            return endpoint;
        }
    }

    // Normal mode: use real Azure Key Vault
    if config.vault_name.starts_with("https://") {
        config.vault_name.clone()
    } else {
        format!("https://{}.vault.azure.net/", config.vault_name)
    }
}

/// Create Azure Key Vault client components
pub async fn create_client_components(
    config: &AzureConfig,
) -> Result<(
    SecretClient,
    ReqwestClient,
    Arc<dyn TokenCredential>,
    String,
)> {
    let vault_url = construct_vault_url(config);
    let credential = create_credential(config)?;

    let client = SecretClient::new(&vault_url, credential.clone(), None)
        .context("Failed to create Azure Key Vault SecretClient")?;

    let http_client = ReqwestClient::builder()
        .build()
        .context("Failed to create HTTP client")?;

    Ok((client, http_client, credential, vault_url))
}
