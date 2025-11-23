//! # AWS Secrets Manager Client
//!
//! Client for interacting with AWS Secrets Manager API.
//!
//! This module provides functionality to:
//! - Create and update secrets in AWS Secrets Manager
//! - Retrieve secret values
//! - Support IRSA (IAM Roles for Service Accounts) authentication

mod auth;
mod operations;
mod pact_api_override;

use aws_sdk_secretsmanager::Client as SecretsManagerClient;

use crate::crd::AwsConfig;
use anyhow::Result;

use self::auth::create_sdk_config;

/// AWS Secrets Manager provider implementation
pub struct AwsSecretManager {
    pub(crate) client: SecretsManagerClient,
    pub(crate) _region: String,
}

impl std::fmt::Debug for AwsSecretManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsSecretManager")
            .field("_region", &self._region)
            .finish_non_exhaustive()
    }
}

impl AwsSecretManager {
    /// Create a new AWS Secrets Manager client
    /// Supports both Access Keys and IRSA (IAM Roles for Service Accounts)
    #[allow(
        clippy::missing_errors_doc,
        reason = "Error documentation is provided in doc comments"
    )]
    pub async fn new(config: &AwsConfig, k8s_client: &kube::Client) -> Result<Self> {
        let region = config.region.clone();
        let sdk_config = create_sdk_config(config, k8s_client).await?;
        let client = SecretsManagerClient::new(&sdk_config);

        Ok(Self {
            client,
            _region: region,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::crd::{AwsAuthConfig, AwsConfig};

    #[test]
    fn test_aws_config_irsa() {
        let config = AwsConfig {
            region: "us-east-1".to_string(),
            auth: Some(AwsAuthConfig::Irsa {
                role_arn: "arn:aws:iam::123456789012:role/test-role".to_string(),
            }),
        };

        assert_eq!(config.region, "us-east-1");
        match config.auth {
            Some(AwsAuthConfig::Irsa { role_arn }) => {
                assert_eq!(role_arn, "arn:aws:iam::123456789012:role/test-role");
            }
            _ => panic!("Expected IRSA auth config"),
        }
    }

    #[test]
    fn test_aws_config_default() {
        let config = AwsConfig {
            region: "eu-west-1".to_string(),
            auth: None,
        };

        assert_eq!(config.region, "eu-west-1");
        assert!(config.auth.is_none());
    }

    #[test]
    fn test_aws_secret_name_validation() {
        // AWS Secrets Manager secret names must be 1-512 characters
        // Can contain letters, numbers, / _ + = . @ -
        let valid_names = vec![
            "my-secret",
            "my/secret/path",
            "my_secret_123",
            "my+secret=test",
            "my.secret@test",
        ];

        for name in valid_names {
            assert!(
                !name.is_empty() && name.len() <= 512,
                "Secret name {name} should be valid"
            );
        }
    }
}
