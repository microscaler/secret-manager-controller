//! GCP Parameter Manager parameter store implementation
//!
//! Wraps the common SecretStore with GCP Parameter Manager-specific behavior:
//! - User-provided version IDs (e.g., "v1234567890")
//! - Parameter key format: "projects/{project}/parameters/{parameter}"

use super::super::common::{SecretStore, SecretVersion};
use serde_json::Value;

/// GCP Parameter Manager-specific parameter store wrapper
#[derive(Clone, Debug)]
pub struct GcpParameterStore {
    store: SecretStore,
}

impl GcpParameterStore {
    pub fn new() -> Self {
        Self {
            store: SecretStore::new(),
        }
    }

    /// Format GCP parameter key
    /// Format: projects/{project}/locations/{location}/parameters/{parameter}
    pub fn format_key(project: &str, location: &str, parameter: &str) -> String {
        format!("projects/{}/locations/{}/parameters/{}", project, location, parameter)
    }

    /// Add a new version to a parameter
    /// Parameter Manager uses user-provided version IDs (e.g., "v1234567890")
    pub async fn add_version(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
        version_data: Value,
        version_id: String, // User-provided version ID (required for Parameter Manager)
    ) -> String {
        let key = Self::format_key(project, location, parameter);
        self.store
            .add_version(
                key,
                version_data,
                Some(version_id.clone()),
                |_store, _key| {
                    // Parameter Manager: version ID is always user-provided
                    // This closure should never be called since version_id is always Some
                    unreachable!("Parameter Manager requires user-provided version IDs")
                },
            )
            .await;
        version_id
    }

    /// Update parameter metadata (format, labels, etc.)
    pub async fn update_metadata(&self, project: &str, location: &str, parameter: &str, metadata: Value) {
        let key = Self::format_key(project, location, parameter);
        self.store.update_metadata(key, metadata).await;
    }

    /// Get the latest version of a parameter
    pub async fn get_latest(&self, project: &str, location: &str, parameter: &str) -> Option<SecretVersion> {
        let key = Self::format_key(project, location, parameter);
        self.store.get_latest(&key).await
    }

    /// Get a specific version by version ID
    pub async fn get_version(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
        version_id: &str,
    ) -> Option<SecretVersion> {
        let key = Self::format_key(project, location, parameter);
        self.store.get_version(&key, version_id).await
    }

    /// Check if a parameter exists
    pub async fn exists(&self, project: &str, location: &str, parameter: &str) -> bool {
        let key = Self::format_key(project, location, parameter);
        self.store.exists(&key).await
    }

    /// Delete a parameter
    pub async fn delete_parameter(&self, project: &str, location: &str, parameter: &str) -> bool {
        let key = Self::format_key(project, location, parameter);
        self.store.delete_secret(&key).await
    }

    /// List all versions of a parameter
    pub async fn list_versions(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
    ) -> Option<Vec<SecretVersion>> {
        let key = Self::format_key(project, location, parameter);
        self.store.list_versions(&key).await
    }

    /// Get parameter metadata
    pub async fn get_metadata(&self, project: &str, location: &str, parameter: &str) -> Option<Value> {
        let key = Self::format_key(project, location, parameter);
        self.store.get_metadata(&key).await
    }

    /// Enable a parameter version
    pub async fn enable_version(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
        version_id: &str,
    ) -> bool {
        let key = Self::format_key(project, location, parameter);
        self.store.enable_version(&key, version_id).await
    }

    /// Disable a parameter version
    pub async fn disable_version(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
        version_id: &str,
    ) -> bool {
        let key = Self::format_key(project, location, parameter);
        self.store.disable_version(&key, version_id).await
    }

    /// Delete a parameter version
    pub async fn delete_version(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
        version_id: &str,
    ) -> bool {
        let key = Self::format_key(project, location, parameter);
        self.store.delete_version(&key, version_id).await
    }
}

