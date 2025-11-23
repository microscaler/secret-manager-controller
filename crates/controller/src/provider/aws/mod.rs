//! # AWS Providers
//!
//! AWS provider modules for secret managers and config stores.
//!
//! - `secrets_manager`: AWS Secrets Manager for secrets
//! - `parameter_store`: AWS Systems Manager Parameter Store for config values

pub mod parameter_store;
mod parameter_store_pact_api_override;
pub mod secrets_manager;

// Re-export for convenience
pub use parameter_store::AwsParameterStore;
pub use secrets_manager::AwsSecretManager;
