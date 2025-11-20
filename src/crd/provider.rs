//! # Provider Configuration
//!
//! Cloud provider configuration types for GCP, AWS, and Azure.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Cloud provider configuration
/// Supports GCP, AWS, and Azure Secret Manager
/// Kubernetes sends data in format: {"type": "gcp", "gcp": {...}}
/// We use externally tagged format and ignore the "type" field during deserialization
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ProviderConfig {
    /// Google Cloud Platform Secret Manager
    #[serde(rename = "gcp")]
    Gcp(GcpConfig),
    /// Amazon Web Services Secrets Manager
    #[serde(rename = "aws")]
    Aws(AwsConfig),
    /// Microsoft Azure Key Vault
    #[serde(rename = "azure")]
    Azure(AzureConfig),
}

impl<'de> serde::Deserialize<'de> for ProviderConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct ProviderConfigVisitor;

        impl<'de> Visitor<'de> for ProviderConfigVisitor {
            type Value = ProviderConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a provider config object with gcp, aws, or azure field")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut gcp: Option<GcpConfig> = None;
                let mut aws: Option<AwsConfig> = None;
                let mut azure: Option<AzureConfig> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "gcp" => {
                            if gcp.is_some() {
                                return Err(de::Error::duplicate_field("gcp"));
                            }
                            // Deserialize GcpConfig from the nested object
                            // The JSON has {"projectId": "..."} which should map to project_id via rename_all
                            gcp = Some(map.next_value::<GcpConfig>().map_err(|e| {
                                de::Error::custom(format!("Failed to deserialize GcpConfig: {e}"))
                            })?);
                        }
                        "aws" => {
                            if aws.is_some() {
                                return Err(de::Error::duplicate_field("aws"));
                            }
                            aws = Some(map.next_value()?);
                        }
                        "azure" => {
                            if azure.is_some() {
                                return Err(de::Error::duplicate_field("azure"));
                            }
                            azure = Some(map.next_value()?);
                        }
                        "type" => {
                            // Ignore the "type" field - it's redundant
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                        _ => {
                            // Ignore unknown fields (like "type")
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                match (gcp, aws, azure) {
                    (Some(config), None, None) => Ok(ProviderConfig::Gcp(config)),
                    (None, Some(config), None) => Ok(ProviderConfig::Aws(config)),
                    (None, None, Some(config)) => Ok(ProviderConfig::Azure(config)),
                    (None, None, None) => Err(de::Error::missing_field("gcp, aws, or azure")),
                    _ => Err(de::Error::custom("multiple provider types specified")),
                }
            }
        }

        deserializer.deserialize_map(ProviderConfigVisitor)
    }
}

/// GCP configuration for Secret Manager
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GcpConfig {
    /// GCP project ID for Secret Manager
    pub project_id: String,
    /// GCP authentication configuration. If not specified, defaults to Workload Identity (recommended).
    #[serde(default)]
    pub auth: Option<GcpAuthConfig>,
}

/// AWS configuration for Secrets Manager
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AwsConfig {
    /// AWS region for Secrets Manager (e.g., "us-east-1", "eu-west-1")
    pub region: String,
    /// AWS authentication configuration. If not specified, defaults to IRSA (IAM Roles for Service Accounts) - recommended.
    #[serde(default)]
    pub auth: Option<AwsAuthConfig>,
}

/// Azure configuration for Key Vault
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AzureConfig {
    /// Azure Key Vault name
    pub vault_name: String,
    /// Azure authentication configuration. If not specified, defaults to Workload Identity (recommended).
    #[serde(default)]
    pub auth: Option<AzureAuthConfig>,
}

/// GCP authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum GcpAuthConfig {
    /// Use Workload Identity for authentication (DEFAULT)
    /// Requires GKE cluster with Workload Identity enabled
    /// This is the recommended authentication method and is used by default when auth is not specified
    WorkloadIdentity {
        /// GCP service account email to impersonate
        /// Format: <service-account-name>@<project-id>.iam.gserviceaccount.com
        service_account_email: String,
    },
}

/// AWS authentication configuration
/// Only supports IRSA (IAM Roles for Service Accounts) - recommended and default
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AwsAuthConfig {
    /// Use IRSA (IAM Roles for Service Accounts) for authentication (DEFAULT)
    /// Requires EKS cluster with IRSA enabled and service account annotation
    /// This is the recommended authentication method and is used by default when auth is not specified
    Irsa {
        /// AWS IAM role ARN to assume
        /// Format: arn:aws:iam::<account-id>:role/<role-name>
        role_arn: String,
    },
}

/// Azure authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AzureAuthConfig {
    /// Use Workload Identity for authentication (DEFAULT)
    /// Requires AKS cluster with Workload Identity enabled
    /// This is the recommended authentication method and is used by default when auth is not specified
    WorkloadIdentity {
        /// Azure service principal client ID
        client_id: String,
    },
}
