//! # OpenTelemetry Configuration
//!
//! OpenTelemetry configuration types for distributed tracing.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// OpenTelemetry configuration
/// Supports both OTLP exporter and Datadog direct export
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum OtelConfig {
    /// Use OTLP exporter to send traces to an OpenTelemetry Collector
    Otlp {
        /// OTLP endpoint URL (e.g., "http://otel-collector:4317")
        endpoint: String,
        /// Service name for traces (defaults to "secret-manager-controller")
        #[serde(default)]
        service_name: Option<String>,
        /// Service version for traces (defaults to Cargo package version)
        #[serde(default)]
        service_version: Option<String>,
        /// Deployment environment (e.g., "dev", "prod")
        #[serde(default)]
        environment: Option<String>,
    },
    /// Use Datadog OpenTelemetry exporter (direct to Datadog)
    Datadog {
        /// Service name for traces (defaults to "secret-manager-controller")
        #[serde(default)]
        service_name: Option<String>,
        /// Service version for traces (defaults to Cargo package version)
        #[serde(default)]
        service_version: Option<String>,
        /// Deployment environment (e.g., "dev", "prod")
        #[serde(default)]
        environment: Option<String>,
        /// Datadog site (e.g., "datadoghq.com", "us3.datadoghq.com")
        /// If not specified, uses DD_SITE environment variable or defaults to "datadoghq.com"
        #[serde(default)]
        site: Option<String>,
        /// Datadog API key
        /// If not specified, uses DD_API_KEY environment variable
        #[serde(default)]
        api_key: Option<String>,
    },
}
