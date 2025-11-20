//! # Azure App Configuration Client
//!
//! Client for interacting with Azure App Configuration REST API.
//!
//! This module provides functionality to:
//! - Create and update key-value pairs in Azure App Configuration
//! - Retrieve configuration values
//! - Support Workload Identity authentication
//!
//! Azure App Configuration is used for storing configuration values (non-secrets)
//! and provides better integration with AKS via Azure App Configuration Kubernetes Provider.

mod auth;
mod client;
mod operations;
mod types;

use crate::crd::AzureConfig;
use crate::provider::ConfigStoreProvider;
use anyhow::Result;
use azure_core::credentials::TokenCredential;
use std::sync::Arc;

use self::auth::create_credential;
use self::client::create_client_components;
use self::operations::AzureAppConfigurationOperations;

/// Azure App Configuration provider implementation
pub struct AzureAppConfiguration {
    pub(crate) operations: AzureAppConfigurationOperations,
}

impl std::fmt::Debug for AzureAppConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureAppConfiguration")
            .field("_endpoint", &self.operations.components.endpoint)
            .field("_key_prefix", &self.operations.components.key_prefix)
            .finish_non_exhaustive()
    }
}

impl AzureAppConfiguration {
    /// Create a new Azure App Configuration client
    /// Supports Workload Identity authentication
    /// # Errors
    /// Returns an error if Azure client initialization fails
    #[allow(
        clippy::missing_errors_doc,
        clippy::unused_async,
        reason = "Error documentation is provided in doc comments, async signature may be needed for future credential initialization"
    )]
    pub async fn new(
        config: &AzureConfig,
        app_config_endpoint: Option<&str>,
        secret_prefix: &str,
        environment: &str,
        _k8s_client: &kube::Client,
    ) -> Result<Self> {
        let credential = create_credential(config)?;
        let components = create_client_components(
            config,
            app_config_endpoint,
            secret_prefix,
            environment,
            credential,
        )?;
        let operations = AzureAppConfigurationOperations { components };
        Ok(Self { operations })
    }
}

#[async_trait::async_trait]
impl ConfigStoreProvider for AzureAppConfiguration {
    async fn create_or_update_config(&self, config_key: &str, config_value: &str) -> Result<bool> {
        self.operations
            .create_or_update_config(config_key, config_value)
            .await
    }

    async fn get_config_value(&self, config_key: &str) -> Result<Option<String>> {
        self.operations.get_config_value(config_key).await
    }

    async fn delete_config(&self, config_key: &str) -> Result<()> {
        self.operations.delete_config(config_key).await
    }
}
