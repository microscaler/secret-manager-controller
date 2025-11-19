//! # Custom Resource Definitions
//!
//! CRD types for the Secret Manager Controller.
//!
//! This module contains all the Kubernetes Custom Resource Definition types
//! used by the controller, including SecretManagerConfig and its related types.

use kube::CustomResource;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// SecretManagerConfig Custom Resource Definition
///
/// This CRD defines the configuration for syncing secrets from GitOps repositories
/// to cloud secret managers (GCP, AWS, Azure).
///
/// # Example
///
/// ```yaml
/// apiVersion: secret-management.microscaler.io/v1
/// kind: SecretManagerConfig
/// metadata:
///   name: my-service-secrets
///   namespace: default
/// spec:
///   sourceRef:
///     kind: GitRepository
///     name: my-repo
///     namespace: microscaler-system
///   provider:
///     gcp:
///       projectId: my-gcp-project
///   secrets:
///     environment: dev
///     kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
/// ```
#[derive(kube::CustomResource, Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[kube(
    kind = "SecretManagerConfig",
    group = "secret-management.microscaler.io",
    version = "v1",
    namespaced,
    status = "SecretManagerConfigStatus",
    shortname = "smc",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}, {"name":"Description", "type":"string", "jsonPath":".status.description"}, {"name":"Ready", "type":"string", "jsonPath":".status.conditions[?(@.type==\"Ready\")].status"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct SecretManagerConfigSpec {
    /// Source reference - supports FluxCD GitRepository and ArgoCD Application
    /// This makes the controller GitOps-agnostic
    pub source_ref: SourceRef,
    /// Cloud provider configuration - supports GCP, AWS, and Azure
    pub provider: ProviderConfig,
    /// Secrets sync configuration
    pub secrets: SecretsConfig,
    /// Config store configuration for routing application.properties to config stores
    /// When enabled, properties are stored individually in config stores instead of as a JSON blob in secret stores
    #[serde(default)]
    pub configs: Option<ConfigsConfig>,
    /// OpenTelemetry configuration for distributed tracing (optional)
    /// Supports OTLP exporter (to OpenTelemetry Collector) and Datadog direct export
    /// If not specified, OpenTelemetry is disabled and standard tracing is used
    #[serde(default)]
    pub otel: Option<OtelConfig>,
    /// GitRepository pull update interval
    /// How often to check for updates from the GitRepository source
    /// Format: Kubernetes duration string (e.g., "1m", "5m", "1h")
    /// Minimum: 1m (60 seconds) - shorter intervals may hit API rate limits
    /// Default: "5m" (5 minutes)
    /// Recommended: 5m or greater to avoid rate limiting
    #[serde(default = "default_git_repository_pull_interval")]
    pub git_repository_pull_interval: String,
    /// Reconcile interval
    /// How often to reconcile secrets between Git and cloud providers (Secret Manager or Parameter Manager)
    /// Format: Kubernetes duration string (e.g., "1m", "30s", "5m")
    /// Default: "1m" (1 minute)
    #[serde(default = "default_reconcile_interval")]
    pub reconcile_interval: String,
    /// Enable diff discovery
    /// When enabled, detects if secrets have been tampered with in Secret Manager or Parameter Manager
    /// and logs warnings when differences are found between Git (source of truth) and cloud provider
    /// Default: true (enabled)
    #[serde(default = "default_true")]
    pub diff_discovery: bool,
    /// Enable update triggers
    /// When enabled, automatically updates cloud provider secrets if Git values have changed since last pull
    /// This ensures Git remains the source of truth
    /// Default: true (enabled)
    #[serde(default = "default_true")]
    pub trigger_update: bool,
    /// Suspend reconciliation
    /// When true, the controller will skip reconciliation for this resource
    /// Useful for troubleshooting or during intricate CI/CD transitions where secrets need to be carefully managed
    /// Manual reconciliation via msmctl will also be blocked when suspended
    /// Default: false (reconciliation enabled)
    #[serde(default = "default_false")]
    pub suspend: bool,
    /// Suspend GitRepository pulls
    /// When true, suspends Git pulls from the referenced GitRepository but continues reconciliation with the last pulled commit
    /// This is useful when you want to freeze the Git state but keep syncing secrets from the current commit
    /// The controller will automatically patch the GitRepository resource to set suspend: true/false
    /// Default: false (Git pulls enabled)
    #[serde(default = "default_false")]
    pub suspend_git_pulls: bool,
}

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

/// Secrets sync configuration
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretsConfig {
    /// Environment/profile name to sync (e.g., "dev", "dev-cf", "prod-cf", "pp-cf")
    /// This must match the directory name under profiles/
    pub environment: String,
    /// Kustomize path - path to kustomization.yaml file (relative to GitRepository root)
    /// If specified, controller will run `kustomize build` on this path and extract secrets
    /// from the generated Kubernetes Secret resources. This supports kustomize overlays,
    /// patches, and generators. Works with any GitOps tool (FluxCD, ArgoCD, etc.)
    /// Examples: "microservices/idam/deployment-configuration/profiles/dev" or "./deployment-configuration/profiles/dev"
    /// If not specified, controller reads raw application.secrets.env files directly
    #[serde(default)]
    pub kustomize_path: Option<String>,
    /// Base path for application files (optional, used only if kustomize_path is not specified)
    /// If not specified, searches from repository root
    /// Examples: "microservices", "services", "apps", or "." for root
    #[serde(default)]
    pub base_path: Option<String>,
    /// Secret name prefix (default: repository name)
    /// Matches kustomize-google-secret-manager prefix behavior
    #[serde(default)]
    pub prefix: Option<String>,
    /// Secret name suffix (optional)
    /// Matches kustomize-google-secret-manager suffix behavior
    /// Common use cases: environment identifiers, tags, etc.
    #[serde(default)]
    pub suffix: Option<String>,
}

/// Config store configuration for routing application.properties to config stores
/// When enabled, properties are stored individually in config stores instead of as a JSON blob in secret stores
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigsConfig {
    /// Enable config store sync (default: false for backward compatibility)
    /// When true, application.properties files are routed to config stores
    /// When false, properties are stored as a JSON blob in secret stores (current behavior)
    #[serde(default)]
    pub enabled: bool,
    /// AWS-specific: Parameter path prefix
    /// Only applies when provider.type == aws
    /// Optional: defaults to /{prefix}/{environment} if not specified
    /// Example: /my-service/dev
    #[serde(default)]
    pub parameter_path: Option<String>,
    /// GCP-specific: Store type (default: SecretManager)
    /// Only applies when provider.type == gcp
    /// - SecretManager: Store configs as individual secrets in Secret Manager (interim solution)
    /// - ParameterManager: Store configs in Parameter Manager (future, after ESO contribution)
    #[serde(default)]
    pub store: Option<ConfigStoreType>,
    /// Azure-specific: App Configuration endpoint
    /// Only applies when provider.type == azure
    /// Optional: defaults to auto-detection from vault region if not specified
    /// Example: https://my-app-config.azconfig.io
    #[serde(default)]
    pub app_config_endpoint: Option<String>,
}

/// GCP config store type
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigStoreType {
    /// Store configs as individual secrets in Secret Manager (interim solution)
    /// This is the default and recommended interim approach until Parameter Manager support is contributed to ESO
    SecretManager,
    /// Store configs in Parameter Manager (future)
    /// Requires ESO contribution for Kubernetes consumption
    #[serde(rename = "ParameterManager")]
    ParameterManager,
}

impl JsonSchema for ConfigStoreType {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("ConfigStoreType")
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        // Generate a structural schema for Kubernetes CRD
        // Use enum with nullable support (not anyOf)
        let schema_value = serde_json::json!({
            "type": "string",
            "enum": ["secretManager", "ParameterManager"],
            "description": "GCP config store type. SecretManager: Store configs as individual secrets in Secret Manager (interim solution). ParameterManager: Store configs in Parameter Manager (future, after ESO contribution)."
        });
        Schema::try_from(schema_value).expect("Failed to create Schema for ConfigStoreType")
    }
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

/// Source reference for GitOps repositories
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    /// Source kind: "GitRepository" (FluxCD) or "Application" (ArgoCD)
    #[serde(default = "default_source_kind")]
    pub kind: String,
    /// Source name
    pub name: String,
    /// Source namespace
    pub namespace: String,
}

fn default_source_kind() -> String {
    "GitRepository".to_string()
}

fn default_git_repository_pull_interval() -> String {
    "5m".to_string()
}

fn default_reconcile_interval() -> String {
    "1m".to_string()
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

/// Status of the SecretManagerConfig resource
///
/// Tracks reconciliation state, errors, and metrics.
#[derive(Debug, Clone, Deserialize, Serialize, Default, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretManagerConfigStatus {
    /// Current phase of reconciliation
    /// Values: Pending, Started, Cloning, Updating, Failed, Ready
    #[serde(default)]
    pub phase: Option<String>,
    /// Human-readable description of current state
    /// Examples: "Clone failed, repo unavailable", "Reconciling secrets to Secret Manager", "Reconciling properties to Parameter Manager"
    #[serde(default)]
    pub description: Option<String>,
    /// Conditions represent the latest available observations
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// Observed generation
    #[serde(default)]
    pub observed_generation: Option<i64>,
    /// Last reconciliation time
    #[serde(default)]
    pub last_reconcile_time: Option<String>,
    /// Next scheduled reconciliation time (RFC3339)
    /// Used to persist periodic reconciliation schedule across watch restarts
    #[serde(default)]
    pub next_reconcile_time: Option<String>,
    /// Number of secrets synced
    #[serde(default)]
    pub secrets_synced: Option<i32>,
    /// SOPS decryption status
    /// Values: Success, TransientFailure, PermanentFailure, NotApplicable
    /// NotApplicable means no SOPS-encrypted files were processed
    #[serde(default)]
    pub decryption_status: Option<String>,
    /// Timestamp of last SOPS decryption attempt (RFC3339)
    /// Updated whenever a SOPS-encrypted file is processed
    #[serde(default)]
    pub last_decryption_attempt: Option<String>,
    /// Last SOPS decryption error message (if any)
    /// Only set when decryption fails
    #[serde(default)]
    pub last_decryption_error: Option<String>,
    /// Whether SOPS private key is available in the resource namespace
    /// Updated when key secret changes (via watch)
    /// Used to avoid redundant API calls on every reconcile
    #[serde(default)]
    pub sops_key_available: Option<bool>,
    /// Name of the SOPS key secret found in the resource namespace
    /// Example: "sops-private-key"
    #[serde(default)]
    pub sops_key_secret_name: Option<String>,
    /// Namespace where the SOPS key was found
    /// Usually the resource namespace, but could be controller namespace if fallback
    #[serde(default)]
    pub sops_key_namespace: Option<String>,
    /// Last time the SOPS key availability was checked (RFC3339)
    #[serde(default)]
    pub sops_key_last_checked: Option<String>,
}

/// Condition represents a condition of a resource
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// Type of condition
    pub r#type: String,
    /// Status of the condition (True, False, Unknown)
    pub status: String,
    /// Last transition time
    #[serde(default)]
    pub last_transition_time: Option<String>,
    /// Reason for the condition
    #[serde(default)]
    pub reason: Option<String>,
    /// Message describing the condition
    #[serde(default)]
    pub message: Option<String>,
}

// Types are already public, no need to re-export
