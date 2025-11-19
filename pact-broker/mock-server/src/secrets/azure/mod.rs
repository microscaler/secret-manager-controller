//! Azure Key Vault secret store implementation
//!
//! Wraps the common SecretStore with Azure-specific behavior:
//! - UUID-like version IDs
//! - Secret key format: secret name (no path prefix)
//! - Each update creates a new version automatically

use super::common::{SecretStore, SecretVersion};
use serde_json::Value;

/// Azure-specific secret store wrapper
#[derive(Clone, Debug)]
pub struct AzureSecretStore {
    store: SecretStore,
}

impl AzureSecretStore {
    pub fn new() -> Self {
        Self {
            store: SecretStore::new(),
        }
    }

    /// Generate UUID-like version ID for Azure
    fn generate_version_id(secret_name: &str, timestamp: u64) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        secret_name.hash(&mut hasher);
        timestamp.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Add a new version to a secret (or create if it doesn't exist)
    /// Azure uses UUID-like version IDs
    pub async fn add_version(&self, secret_name: &str, version_data: Value, version_id: Option<String>) -> String {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let new_version_id = version_id.unwrap_or_else(|| {
            Self::generate_version_id(secret_name, timestamp)
        });

        self.store.add_version(
            secret_name.to_string(),
            version_data,
            Some(new_version_id.clone()),
            |_, _| new_version_id.clone(), // Not used since we provide version_id
        ).await
    }

    /// Set/update secret (creates new version automatically)
    /// This is the main method for Azure - each call creates a new version
    pub async fn set_secret(&self, secret_name: &str, value: String) -> String {
        let version_data = serde_json::json!({
            "value": value
        });
        self.add_version(secret_name, version_data, None).await
    }

    /// Get the latest version of a secret
    pub async fn get_latest(&self, secret_name: &str) -> Option<SecretVersion> {
        self.store.get_latest(secret_name).await
    }

    /// Get a specific version by version ID
    pub async fn get_version(&self, secret_name: &str, version_id: &str) -> Option<SecretVersion> {
        self.store.get_version(secret_name, version_id).await
    }

    /// List all versions of a secret
    pub async fn list_versions(&self, secret_name: &str) -> Option<Vec<SecretVersion>> {
        self.store.list_versions(secret_name).await
    }

    /// Get secret metadata
    pub async fn get_metadata(&self, secret_name: &str) -> Option<Value> {
        self.store.get_metadata(secret_name).await
    }

    /// Delete a secret (all versions)
    pub async fn delete_secret(&self, secret_name: &str) -> bool {
        self.store.delete_secret(secret_name).await
    }

    /// Check if a secret exists
    pub async fn exists(&self, secret_name: &str) -> bool {
        self.store.exists(secret_name).await
    }

    /// Disable a secret (disables all versions, but keeps them for history)
    pub async fn disable_secret(&self, secret_name: &str) -> bool {
        self.store.disable_secret(secret_name).await
    }

    /// Enable a secret (re-enables the secret, versions remain in their current state)
    pub async fn enable_secret(&self, secret_name: &str) -> bool {
        self.store.enable_secret(secret_name).await
    }

    /// Disable a specific version
    pub async fn disable_version(&self, secret_name: &str, version_id: &str) -> bool {
        self.store.disable_version(secret_name, version_id).await
    }

    /// Enable a specific version
    pub async fn enable_version(&self, secret_name: &str, version_id: &str) -> bool {
        self.store.enable_version(secret_name, version_id).await
    }

    /// Check if a secret is enabled
    pub async fn is_enabled(&self, secret_name: &str) -> bool {
        self.store.is_enabled(secret_name).await
    }

    /// List all secret names
    pub async fn list_all_secrets(&self) -> Vec<String> {
        self.store.list_all_keys().await
    }
}

impl Default for AzureSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

