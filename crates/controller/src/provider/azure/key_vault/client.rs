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
/// In Pact mode, uses the endpoint from PactModeAPIOverride
pub fn construct_vault_url(config: &AzureConfig) -> String {
    // CRITICAL: Override API endpoint BEFORE creating client
    let endpoint_override = {
        // Check if PACT_MODE is enabled (drop guard immediately)
        let enabled = {
            let pact_config = crate::config::PactModeConfig::get();
            let enabled = pact_config.enabled;
            drop(pact_config); // Drop guard before calling override_api_endpoint
            enabled
        };

        if enabled {
            use crate::config::PactModeAPIOverride;
            use crate::provider::azure::key_vault::pact_api_override::AzureKeyVaultAPIOverride;

            let api_override = AzureKeyVaultAPIOverride;
            if let Err(e) = api_override.override_api_endpoint() {
                // Log error but continue - endpoint might be set via env var
                tracing::warn!("Failed to override Azure Key Vault API endpoint: {}", e);
            }

            // Get endpoint (this will get the config again, but guard is dropped)
            api_override.get_endpoint()
        } else {
            None
        }
    };

    // Use override endpoint if available
    if let Some(endpoint) = endpoint_override {
        info!(
            "PACT_MODE: Routing Azure Key Vault requests to {}",
            endpoint
        );
        return endpoint;
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
