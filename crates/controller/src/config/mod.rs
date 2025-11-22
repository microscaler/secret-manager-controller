//! # Controller Configuration
//!
//! Controller-level configuration loaded from environment variables (populated from ConfigMap).
//!
//! All configuration has sensible defaults and can be overridden via environment variables.
//! Environment variables are populated from a ConfigMap using `envFrom` in the deployment.
//!
//! The configuration can be hot-reloaded by watching the ConfigMap for changes.

mod controller;
mod server;
mod watch;

pub use controller::ControllerConfig;
pub use server::ServerConfig;
pub use watch::start_configmap_watch;

use std::sync::Arc;
use tokio::sync::RwLock;

/// Global controller configuration
/// This is updated when the ConfigMap changes (hot-reload)
pub type SharedControllerConfig = Arc<RwLock<ControllerConfig>>;

/// Global server configuration
/// This is updated when the ConfigMap changes (hot-reload)
pub type SharedServerConfig = Arc<RwLock<ServerConfig>>;

/// Load configuration from environment variables with defaults
pub fn load_config() -> (ControllerConfig, ServerConfig) {
    (ControllerConfig::from_env(), ServerConfig::from_env())
}

/// Create shared configuration instances
pub fn create_shared_config() -> (SharedControllerConfig, SharedServerConfig) {
    let (controller_config, server_config) = load_config();
    (
        Arc::new(RwLock::new(controller_config)),
        Arc::new(RwLock::new(server_config)),
    )
}
