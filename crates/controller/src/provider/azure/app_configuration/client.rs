//! # Azure App Configuration Client Creation
//!
//! Handles the creation of HTTP client and endpoint configuration.

use crate::crd::AzureConfig;
use anyhow::{Context, Result};
use azure_core::credentials::TokenCredential;
use reqwest::Client;
use std::sync::Arc;
use tracing::info;

/// Components needed for Azure App Configuration operations
pub struct ClientComponents {
    pub http_client: Client,
    pub endpoint: String,
    pub credential: Arc<dyn TokenCredential>,
    pub key_prefix: String,
}

/// Create Azure App Configuration client components
pub fn create_client_components(
    config: &AzureConfig,
    app_config_endpoint: Option<&str>,
    secret_prefix: &str,
    environment: &str,
    credential: Arc<dyn TokenCredential>,
) -> Result<ClientComponents> {
    // Construct App Configuration endpoint
    // Format: https://{store-name}.azconfig.io
    let endpoint = if let Some(endpoint) = app_config_endpoint {
        endpoint.to_string()
    } else {
        // Auto-detect from vault name (assume same region/resource group)
        // Extract store name from vault name pattern
        // This is a simple heuristic - users should provide endpoint explicitly
        let store_name = config.vault_name.replace("-vault", "-appconfig");
        format!("https://{store_name}.azconfig.io")
    };

    // Ensure endpoint doesn't have trailing slash
    let endpoint = endpoint.trim_end_matches('/').to_string();

    info!("Azure App Configuration endpoint: {}", endpoint);

    // Create HTTP client with rustls
    let http_client = Client::builder()
        .build()
        .context("Failed to create HTTP client")?;

    // Construct key prefix: {prefix}:{environment}:
    // Azure App Configuration uses colon-separated keys
    let key_prefix = format!("{secret_prefix}:{environment}:");

    Ok(ClientComponents {
        http_client,
        endpoint,
        credential,
        key_prefix,
    })
}
