//! # Azure Providers
//!
//! Azure provider modules for secret managers and config stores.
//!
//! - `key_vault`: Azure Key Vault for secrets
//! - `app_configuration`: Azure App Configuration for config values

pub mod key_vault;
pub mod app_configuration;

// Re-export for convenience
pub use key_vault::AzureKeyVault;
pub use app_configuration::AzureAppConfiguration;
