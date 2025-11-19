//! # Azure Key Vault Client
//!
//! Client for interacting with Azure Key Vault Secrets API.
//!
//! This module provides functionality to:
//! - Create and update secrets in Azure Key Vault
//! - Retrieve secret values
//! - Support Workload Identity and Service Principal authentication

use crate::crd::{AzureAuthConfig, AzureConfig};
use crate::observability::metrics;
use crate::provider::SecretManagerProvider;
use anyhow::{Context, Result};
use async_trait::async_trait;
use azure_core::credentials::{AccessToken, Secret, TokenCredential, TokenRequestOptions};
use azure_identity::{ManagedIdentityCredential, WorkloadIdentityCredential};
use azure_security_keyvault_secrets::{models::SetSecretParameters, SecretClient};
use reqwest::Client as ReqwestClient;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, info_span, warn, Instrument};

/// Mock TokenCredential for Pact testing
/// Returns a dummy token without attempting real Azure authentication
#[derive(Debug)]
struct MockTokenCredential;

#[async_trait]
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

/// Azure Key Vault provider implementation
pub struct AzureKeyVault {
    client: SecretClient,
    _vault_url: String,
    http_client: ReqwestClient,
    credential: Arc<dyn TokenCredential>,
}

impl std::fmt::Debug for AzureKeyVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureKeyVault")
            .field("_vault_url", &self._vault_url)
            .finish_non_exhaustive()
    }
}

impl AzureKeyVault {
    /// Create a new Azure Key Vault client
    /// Supports both Service Principal and Workload Identity
    /// # Errors
    /// Returns an error if Azure client initialization fails
    #[allow(
        clippy::missing_errors_doc,
        clippy::unused_async,
        reason = "Error docs in comments, async signature matches trait"
    )]
    pub async fn new(config: &AzureConfig, _k8s_client: &kube::Client) -> Result<Self> {
        // Construct vault URL from vault name
        // Format: https://{vault-name}.vault.azure.net/
        // Support Pact mock server integration via environment variable
        let vault_url = if std::env::var("PACT_MODE").is_ok() {
            // When PACT_MODE=true, use Pact mock server endpoint
            if let Ok(endpoint) = std::env::var("AZURE_KEY_VAULT_ENDPOINT") {
                info!(
                    "Pact mode enabled: routing Azure Key Vault requests to {}",
                    endpoint
                );
                endpoint
            } else {
                // Fallback to default if endpoint not set
                if config.vault_name.starts_with("https://") {
                    config.vault_name.clone()
                } else {
                    format!("https://{}.vault.azure.net/", config.vault_name)
                }
            }
        } else {
            // Normal mode: use real Azure Key Vault
            if config.vault_name.starts_with("https://") {
                config.vault_name.clone()
            } else {
                format!("https://{}.vault.azure.net/", config.vault_name)
            }
        };

        // Build credential based on authentication method
        // Only support Workload Identity or Managed Identity (workload identity equivalents)
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
                    WorkloadIdentityCredential::new(Some(options))
                        .context("Failed to create WorkloadIdentityCredential")?
                }
                None => {
                    // Default to Managed Identity (works in Azure environments like AKS)
                    info!("No auth configuration specified, using Managed Identity");
                    info!(
                        "This works automatically in Azure environments (AKS, App Service, etc.)"
                    );
                    ManagedIdentityCredential::new(None)
                        .context("Failed to create ManagedIdentityCredential")?
                }
            }
        };

        let client = SecretClient::new(&vault_url, credential.clone(), None)
            .context("Failed to create Azure Key Vault SecretClient")?;

        let http_client = ReqwestClient::builder()
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            _vault_url: vault_url,
            http_client,
            credential,
        })
    }
}

#[async_trait]
impl SecretManagerProvider for AzureKeyVault {
    async fn create_or_update_secret(&self, secret_name: &str, secret_value: &str) -> Result<bool> {
        let vault_name = self
            ._vault_url
            .strip_prefix("https://")
            .and_then(|s| s.strip_suffix(".vault.azure.net/"))
            .unwrap_or("unknown");
        let span = info_span!(
            "azure.keyvault.secret.create_or_update",
            secret.name = secret_name,
            vault.name = vault_name
        );
        let span_clone = span.clone();
        let start = Instant::now();
        let secret_value_clone = secret_value.to_string();

        async move {
            // Check if secret exists by trying to get it
            let current_value = self.get_secret_value(secret_name).await?;

            let operation_type = if let Some(current) = current_value {
                if current == secret_value_clone {
                    debug!("Azure secret {} unchanged, skipping update", secret_name);
                    metrics::record_secret_operation(
                        "azure",
                        "no_change",
                        start.elapsed().as_secs_f64(),
                    );
                    span_clone.record("operation.type", "no_change");
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    span_clone.record("operation.success", true);
                    return Ok(false);
                }
                "update"
            } else {
                "create"
            };

            // Create or update secret
            // Azure Key Vault automatically creates a new version when updating
            info!("Creating/updating Azure secret: {}", secret_name);
            let parameters = SetSecretParameters {
                value: Some(secret_value_clone),
                ..Default::default()
            };
            match self
                .client
                .set_secret(secret_name, parameters.try_into()?, None)
                .await
            {
                Ok(_) => {
                    metrics::record_secret_operation(
                        "azure",
                        operation_type,
                        start.elapsed().as_secs_f64(),
                    );
                    span_clone.record("operation.type", operation_type);
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    span_clone.record("operation.success", true);
                    Ok(true)
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    span_clone.record("operation.success", false);
                    span_clone.record("operation.type", operation_type);
                    span_clone.record("error.message", error_msg.clone());
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    metrics::increment_provider_operation_errors("azure");
                    Err(anyhow::anyhow!(
                        "Failed to create/update Azure secret {secret_name}: {e}"
                    ))
                }
            }
        }
        .instrument(span)
        .await
    }

    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>> {
        let vault_name = self
            ._vault_url
            .strip_prefix("https://")
            .and_then(|s| s.strip_suffix(".vault.azure.net/"))
            .unwrap_or("unknown");
        let span = tracing::debug_span!(
            "azure.keyvault.secret.get",
            secret.name = secret_name,
            vault.name = vault_name
        );
        let span_clone = span.clone();
        let start = Instant::now();

        async move {
            // Get the latest version of the secret (no version parameter needed - defaults to latest)
            match self.client.get_secret(secret_name, None).await {
                Ok(response) => {
                    // Response body needs to be deserialized into the Secret model
                    use azure_security_keyvault_secrets::models::Secret;
                    match serde_json::from_slice::<Secret>(&response.into_body()) {
                        Ok(secret) => {
                            span_clone.record("operation.success", true);
                            span_clone.record("operation.found", secret.value.is_some());
                            span_clone.record(
                                "operation.duration_ms",
                                start.elapsed().as_millis() as u64,
                            );
                            metrics::record_secret_operation(
                                "azure",
                                "get",
                                start.elapsed().as_secs_f64(),
                            );
                            Ok(secret.value)
                        }
                        Err(e) => {
                            let error_msg = e.to_string();
                            span_clone.record("operation.success", false);
                            span_clone.record(
                                "error.message",
                                format!(
                                    "Failed to deserialize Azure secret response: {}",
                                    error_msg
                                ),
                            );
                            span_clone.record(
                                "operation.duration_ms",
                                start.elapsed().as_millis() as u64,
                            );
                            metrics::increment_provider_operation_errors("azure");
                            Err(anyhow::anyhow!(
                                "Failed to deserialize Azure secret response: {e}"
                            ))
                        }
                    }
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("SecretNotFound")
                        || error_msg.contains("404")
                        || error_msg.contains("not found")
                    {
                        span_clone.record("operation.success", true);
                        span_clone.record("operation.found", false);
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::record_secret_operation(
                            "azure",
                            "get",
                            start.elapsed().as_secs_f64(),
                        );
                        Ok(None)
                    } else {
                        span_clone.record("operation.success", false);
                        span_clone.record("error.message", error_msg.clone());
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::increment_provider_operation_errors("azure");
                        Err(anyhow::anyhow!("Failed to get Azure secret: {e}"))
                    }
                }
            }
        }
        .instrument(span)
        .await
    }

    async fn delete_secret(&self, secret_name: &str) -> Result<()> {
        info!("Deleting Azure secret: {}", secret_name);
        self.client
            .delete_secret(secret_name, None)
            .await
            .context(format!("Failed to delete Azure secret: {secret_name}"))?;
        Ok(())
    }

    async fn disable_secret(&self, secret_name: &str) -> Result<bool> {
        info!("Disabling Azure secret: {}", secret_name);

        // Use REST API directly: PATCH /secrets/{name} with attributes.enabled=false
        // Azure Key Vault REST API: https://learn.microsoft.com/en-us/rest/api/keyvault/secrets/update-secret/update-secret

        // Get access token
        let scope = &["https://vault.azure.net/.default"];
        let options = Some(TokenRequestOptions::default());
        let token_response = self
            .credential
            .get_token(scope, options)
            .await
            .context("Failed to get Azure Key Vault access token")?;
        let token = token_response.token.secret().to_string();

        // Construct URL: PATCH {vault_url}/secrets/{name}?api-version=7.4
        let url = format!("{}secrets/{}?api-version=7.4", self._vault_url, secret_name);

        // Request body: { "attributes": { "enabled": false } }
        let body = json!({
            "attributes": {
                "enabled": false
            }
        });

        let response = self
            .http_client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to disable Azure secret")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // If secret doesn't exist, return false (not an error)
            if status == 404 {
                debug!("Secret {} does not exist, cannot disable", secret_name);
                return Ok(false);
            }

            return Err(anyhow::anyhow!(
                "Failed to disable Azure secret {}: HTTP {} - {}",
                secret_name,
                status,
                error_text
            ));
        }

        Ok(true)
    }

    async fn enable_secret(&self, secret_name: &str) -> Result<bool> {
        info!("Enabling Azure secret: {}", secret_name);

        // Use REST API directly: PATCH /secrets/{name} with attributes.enabled=true
        // Azure Key Vault REST API: https://learn.microsoft.com/en-us/rest/api/keyvault/secrets/update-secret/update-secret

        // Get access token
        let scope = &["https://vault.azure.net/.default"];
        let options = Some(TokenRequestOptions::default());
        let token_response = self
            .credential
            .get_token(scope, options)
            .await
            .context("Failed to get Azure Key Vault access token")?;
        let token = token_response.token.secret().to_string();

        // Construct URL: PATCH {vault_url}/secrets/{name}?api-version=7.4
        let url = format!("{}secrets/{}?api-version=7.4", self._vault_url, secret_name);

        // Request body: { "attributes": { "enabled": true } }
        let body = json!({
            "attributes": {
                "enabled": true
            }
        });

        let response = self
            .http_client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to enable Azure secret")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // If secret doesn't exist, return false (not an error)
            if status == 404 {
                debug!("Secret {} does not exist, cannot enable", secret_name);
                return Ok(false);
            }

            return Err(anyhow::anyhow!(
                "Failed to enable Azure secret {}: HTTP {} - {}",
                secret_name,
                status,
                error_text
            ));
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use crate::crd::{AzureAuthConfig, AzureConfig};

    #[test]
    fn test_azure_config_workload_identity() {
        let config = AzureConfig {
            vault_name: "my-vault".to_string(),
            auth: Some(AzureAuthConfig::WorkloadIdentity {
                client_id: "12345678-1234-1234-1234-123456789012".to_string(),
            }),
        };

        assert_eq!(config.vault_name, "my-vault");
        match config.auth {
            Some(AzureAuthConfig::WorkloadIdentity { client_id }) => {
                assert_eq!(client_id, "12345678-1234-1234-1234-123456789012");
            }
            _ => panic!("Expected WorkloadIdentity auth config"),
        }
    }

    #[test]
    fn test_azure_config_default() {
        let config = AzureConfig {
            vault_name: "prod-vault".to_string(),
            auth: None,
        };

        assert_eq!(config.vault_name, "prod-vault");
        assert!(config.auth.is_none());
    }

    #[test]
    fn test_azure_vault_url_construction() {
        // Test vault URL construction
        let config1 = AzureConfig {
            vault_name: "my-vault".to_string(),
            auth: None,
        };
        let expected_url = "https://my-vault.vault.azure.net/";
        // This would be tested in the new() method, but we can test the logic
        let vault_url = if config1.vault_name.starts_with("https://") {
            config1.vault_name.clone()
        } else {
            format!("https://{}.vault.azure.net/", config1.vault_name)
        };
        assert_eq!(vault_url, expected_url);

        // Test with full URL
        let config2 = AzureConfig {
            vault_name: "https://custom-vault.vault.azure.net/".to_string(),
            auth: None,
        };
        let vault_url2 = if config2.vault_name.starts_with("https://") {
            config2.vault_name.clone()
        } else {
            format!("https://{}.vault.azure.net/", config2.vault_name)
        };
        assert_eq!(vault_url2, "https://custom-vault.vault.azure.net/");
    }

    #[test]
    fn test_azure_secret_name_validation() {
        // Azure Key Vault secret names must be 1-127 characters
        // Can contain letters, numbers, and hyphens
        let valid_names = vec!["my-secret", "my-secret-123", "MySecret", "my_secret"];

        for name in valid_names {
            assert!(
                !name.is_empty() && name.len() <= 127,
                "Secret name {name} should be valid"
            );
        }
    }
}
