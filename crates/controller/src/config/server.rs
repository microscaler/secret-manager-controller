//! # Server Configuration
//!
//! HTTP server settings loaded from environment variables.

/// HTTP server configuration
///
/// All settings have sensible defaults and can be overridden via environment variables.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// HTTP server port for metrics and health probes
    pub metrics_port: u16,
    /// Server startup timeout (seconds)
    /// How long to wait for server to be ready before giving up
    pub startup_timeout_secs: u64,
    /// Server readiness poll interval (milliseconds)
    /// How often to check if server is ready during startup
    pub poll_interval_ms: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        use crate::constants::*;
        Self {
            metrics_port: DEFAULT_METRICS_PORT,
            startup_timeout_secs: DEFAULT_SERVER_STARTUP_TIMEOUT_SECS,
            poll_interval_ms: DEFAULT_SERVER_POLL_INTERVAL_MS,
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables with defaults
    pub fn from_env() -> Self {
        use crate::constants::*;
        Self {
            metrics_port: env_var_or_default("METRICS_PORT", DEFAULT_METRICS_PORT),
            startup_timeout_secs: env_var_or_default(
                "SERVER_STARTUP_TIMEOUT_SECS",
                DEFAULT_SERVER_STARTUP_TIMEOUT_SECS,
            ),
            poll_interval_ms: env_var_or_default(
                "SERVER_POLL_INTERVAL_MS",
                DEFAULT_SERVER_POLL_INTERVAL_MS,
            ),
        }
    }
}

/// Read environment variable or return default value
fn env_var_or_default<T: std::str::FromStr>(key: &str, default: T) -> T
where
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
