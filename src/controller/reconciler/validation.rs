//! # Validation
//!
//! Validates SecretManagerConfig resources and duration strings.

use crate::crd::{ProviderConfig, SecretManagerConfig};
use anyhow::Result;
use regex::Regex;
use std::time::Duration;

/// Parse Kubernetes duration string into std::time::Duration
/// Supports formats: "30s", "1m", "5m", "1h", "2h", "1d"
/// Returns Duration or error if format is invalid
pub fn parse_kubernetes_duration(duration_str: &str) -> Result<Duration> {
    let duration_trimmed = duration_str.trim();

    if duration_trimmed.is_empty() {
        return Err(anyhow::anyhow!("Duration string cannot be empty"));
    }

    // Regex pattern for Kubernetes duration format
    // Matches: <number><unit> where:
    //   - number: one or more digits
    //   - unit: s, m, h, d (case insensitive)
    let duration_regex = Regex::new(r"^(?P<number>\d+)(?P<unit>[smhd])$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    // Match against trimmed, lowercase version
    let interval_lower = duration_trimmed.to_lowercase();

    let captures = duration_regex
        .captures(&interval_lower)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid duration format '{}'. Expected format: <number><unit> (e.g., '1m', '5m', '1h')",
                duration_trimmed
            )
        })?;

    // Extract number and unit from regex captures
    let number_str = captures
        .name("number")
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to extract number from duration '{}'",
                duration_trimmed
            )
        })?
        .as_str();

    let unit = captures
        .name("unit")
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to extract unit from duration '{}'",
                duration_trimmed
            )
        })?
        .as_str();

    // Parse number safely
    let number: u64 = number_str.parse().map_err(|e| {
        anyhow::anyhow!(
            "Invalid duration number '{}' in '{}': {}",
            number_str,
            duration_trimmed,
            e
        )
    })?;

    if number == 0 {
        return Err(anyhow::anyhow!(
            "Duration number must be greater than 0, got '{}'",
            duration_trimmed
        ));
    }

    // Convert to seconds based on unit
    let seconds = match unit {
        "s" => number,
        "m" => number * 60,
        "h" => number * 3600,
        "d" => number * 86400,
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid unit '{}' in duration '{}'. Expected: s, m, h, or d",
                unit,
                duration_trimmed
            ));
        }
    };

    Ok(Duration::from_secs(seconds))
}

/// Validate duration interval with regex and minimum value check
/// Ensures interval matches Kubernetes duration format and meets minimum requirement
/// Accepts Kubernetes duration format: "1m", "5m", "1h", etc.
///
/// # Arguments
/// * `interval` - The duration string to validate
/// * `field_name` - Name of the field for error messages
/// * `min_seconds` - Minimum allowed duration in seconds
pub fn validate_duration_interval(
    interval: &str,
    field_name: &str,
    min_seconds: u64,
) -> Result<()> {
    // Trim whitespace
    let interval_trimmed = interval.trim();

    if interval_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    // Parse the duration to validate format
    let duration = parse_kubernetes_duration(interval_trimmed)?;

    // Check minimum duration
    if duration.as_secs() < min_seconds {
        let min_duration = if min_seconds == 60 {
            "1 minute (60 seconds)"
        } else {
            &format!("{min_seconds} seconds")
        };
        return Err(anyhow::anyhow!(
            "{field_name} must be at least {min_duration} to avoid API rate limits. Got: '{}' ({} seconds)",
            interval_trimmed,
            duration.as_secs()
        ));
    }

    Ok(())
}

/// Comprehensive validation of SecretManagerConfig fields
/// Validates all fields according to CRD schema and Kubernetes conventions
/// Returns Ok(()) if valid, Err with descriptive message if invalid
pub fn validate_secret_manager_config(config: &SecretManagerConfig) -> Result<()> {
    // Validate sourceRef.kind
    if config.spec.source_ref.kind.is_empty() {
        return Err(anyhow::anyhow!("sourceRef.kind is required but is empty"));
    }
    if let Err(e) = validate_source_ref_kind(&config.spec.source_ref.kind) {
        return Err(anyhow::anyhow!(
            "Invalid sourceRef.kind '{}': {}",
            config.spec.source_ref.kind,
            e
        ));
    }

    // Validate sourceRef.name
    if config.spec.source_ref.name.is_empty() {
        return Err(anyhow::anyhow!("sourceRef.name is required but is empty"));
    }
    if let Err(e) = validate_kubernetes_name(&config.spec.source_ref.name, "sourceRef.name") {
        return Err(anyhow::anyhow!(
            "Invalid sourceRef.name '{}': {}",
            config.spec.source_ref.name,
            e
        ));
    }

    // Validate sourceRef.namespace
    if config.spec.source_ref.namespace.is_empty() {
        return Err(anyhow::anyhow!(
            "sourceRef.namespace is required but is empty"
        ));
    }
    if let Err(e) = validate_kubernetes_namespace(&config.spec.source_ref.namespace) {
        return Err(anyhow::anyhow!(
            "Invalid sourceRef.namespace '{}': {}",
            config.spec.source_ref.namespace,
            e
        ));
    }

    // Validate secrets.environment
    if config.spec.secrets.environment.is_empty() {
        return Err(anyhow::anyhow!(
            "secrets.environment is required but is empty"
        ));
    }
    if let Err(e) =
        validate_kubernetes_label(&config.spec.secrets.environment, "secrets.environment")
    {
        return Err(anyhow::anyhow!(
            "Invalid secrets.environment '{}': {}",
            config.spec.secrets.environment,
            e
        ));
    }

    // Validate optional secrets fields
    if let Some(ref prefix) = config.spec.secrets.prefix {
        if !prefix.is_empty() {
            if let Err(e) = validate_secret_name_component(prefix, "secrets.prefix") {
                return Err(anyhow::anyhow!("Invalid secrets.prefix '{prefix}': {e}"));
            }
        }
    }

    if let Some(ref suffix) = config.spec.secrets.suffix {
        if !suffix.is_empty() {
            if let Err(e) = validate_secret_name_component(suffix, "secrets.suffix") {
                return Err(anyhow::anyhow!("Invalid secrets.suffix '{suffix}': {e}"));
            }
        }
    }

    if let Some(ref base_path) = config.spec.secrets.base_path {
        if !base_path.is_empty() {
            if let Err(e) = validate_path(base_path, "secrets.basePath") {
                return Err(anyhow::anyhow!(
                    "Invalid secrets.basePath '{base_path}': {e}"
                ));
            }
        }
    }

    if let Some(ref kustomize_path) = config.spec.secrets.kustomize_path {
        if !kustomize_path.is_empty() {
            if let Err(e) = validate_path(kustomize_path, "secrets.kustomizePath") {
                return Err(anyhow::anyhow!(
                    "Invalid secrets.kustomizePath '{kustomize_path}': {e}"
                ));
            }
        }
    }

    // Validate provider configuration
    if let Err(e) = validate_provider_config(&config.spec.provider) {
        return Err(anyhow::anyhow!("Invalid provider configuration: {e}"));
    }

    // Validate configs configuration if present
    if let Some(ref configs) = config.spec.configs {
        if let Err(e) = validate_configs_config(configs) {
            return Err(anyhow::anyhow!("Invalid configs configuration: {e}"));
        }
    }

    // Boolean fields are validated by serde, but we ensure they're not None
    // diffDiscovery and triggerUpdate have defaults, so they're always present

    Ok(())
}

/// Validate sourceRef.kind
/// Must be "GitRepository" or "Application" (case-sensitive)
fn validate_source_ref_kind(kind: &str) -> Result<()> {
    let kind_trimmed = kind.trim();
    match kind_trimmed {
        "GitRepository" | "Application" => Ok(()),
        _ => Err(anyhow::anyhow!(
            "Must be 'GitRepository' or 'Application' (case-sensitive), got '{kind_trimmed}'"
        )),
    }
}

/// Validate Kubernetes resource name (RFC 1123 subdomain)
/// Format: lowercase alphanumeric, hyphens, dots
/// Length: 1-253 characters
/// Cannot start or end with hyphen or dot
fn validate_kubernetes_name(name: &str, field_name: &str) -> Result<()> {
    let name_trimmed = name.trim();

    if name_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    if name_trimmed.len() > 253 {
        return Err(anyhow::anyhow!(
            "{} '{}' exceeds maximum length of 253 characters (got {})",
            field_name,
            name_trimmed,
            name_trimmed.len()
        ));
    }

    // RFC 1123 subdomain: [a-z0-9]([-a-z0-9]*[a-z0-9])?(\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*
    // Simplified: lowercase alphanumeric, hyphens, dots; cannot start/end with hyphen or dot
    let name_regex =
        Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?(\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !name_regex.is_match(name_trimmed) {
        return Err(anyhow::anyhow!(
            "{field_name} '{name_trimmed}' must be a valid Kubernetes name (lowercase alphanumeric, hyphens, dots; cannot start/end with hyphen or dot)"
        ));
    }

    Ok(())
}

/// Validate Kubernetes namespace (RFC 1123 label)
/// Format: lowercase alphanumeric, hyphens
/// Length: 1-63 characters
/// Cannot start or end with hyphen
fn validate_kubernetes_namespace(namespace: &str) -> Result<()> {
    let namespace_trimmed = namespace.trim();

    if namespace_trimmed.is_empty() {
        return Err(anyhow::anyhow!("sourceRef.namespace cannot be empty"));
    }

    if namespace_trimmed.len() > 63 {
        return Err(anyhow::anyhow!(
            "sourceRef.namespace '{}' exceeds maximum length of 63 characters (got {})",
            namespace_trimmed,
            namespace_trimmed.len()
        ));
    }

    // RFC 1123 label: [a-z0-9]([-a-z0-9]*[a-z0-9])?
    let namespace_regex = Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !namespace_regex.is_match(namespace_trimmed) {
        return Err(anyhow::anyhow!(
            "sourceRef.namespace '{namespace_trimmed}' must be a valid Kubernetes namespace (lowercase alphanumeric, hyphens; cannot start/end with hyphen)"
        ));
    }

    Ok(())
}

/// Validate Kubernetes label value
/// Format: lowercase alphanumeric, hyphens, dots, underscores
/// Length: 1-63 characters
/// Cannot start or end with dot
fn validate_kubernetes_label(label: &str, field_name: &str) -> Result<()> {
    let label_trimmed = label.trim();

    if label_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    if label_trimmed.len() > 63 {
        return Err(anyhow::anyhow!(
            "{} '{}' exceeds maximum length of 63 characters (got {})",
            field_name,
            label_trimmed,
            label_trimmed.len()
        ));
    }

    // Kubernetes label: [a-z0-9]([-a-z0-9_.]*[a-z0-9])?
    let label_regex = Regex::new(r"^[a-z0-9]([-a-z0-9_.]*[a-z0-9])?$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !label_regex.is_match(label_trimmed) {
        return Err(anyhow::anyhow!(
            "{field_name} '{label_trimmed}' must be a valid Kubernetes label (lowercase alphanumeric, hyphens, dots, underscores; cannot start/end with dot)"
        ));
    }

    Ok(())
}

/// Validate secret name component (prefix or suffix)
/// Must be valid for cloud provider secret names
/// Format: alphanumeric, hyphens, underscores
/// Length: 1-255 characters
fn validate_secret_name_component(component: &str, field_name: &str) -> Result<()> {
    let component_trimmed = component.trim();

    if component_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    if component_trimmed.len() > 255 {
        return Err(anyhow::anyhow!(
            "{} '{}' exceeds maximum length of 255 characters (got {})",
            field_name,
            component_trimmed,
            component_trimmed.len()
        ));
    }

    // Secret name component: alphanumeric, hyphens, underscores
    let secret_regex = Regex::new(r"^[a-zA-Z0-9_-]+$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !secret_regex.is_match(component_trimmed) {
        return Err(anyhow::anyhow!(
            "{field_name} '{component_trimmed}' must contain only alphanumeric characters, hyphens, and underscores"
        ));
    }

    Ok(())
}

/// Validate file path
/// Must be a valid relative or absolute path
/// Cannot contain null bytes or invalid path characters
fn validate_path(path: &str, field_name: &str) -> Result<()> {
    let path_trimmed = path.trim();

    if path_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    // Check for null bytes
    if path_trimmed.contains('\0') {
        return Err(anyhow::anyhow!(
            "{field_name} '{path_trimmed}' cannot contain null bytes"
        ));
    }

    // Basic path validation: no control characters, reasonable length
    if path_trimmed.len() > 4096 {
        return Err(anyhow::anyhow!(
            "{} '{}' exceeds maximum length of 4096 characters (got {})",
            field_name,
            path_trimmed,
            path_trimmed.len()
        ));
    }

    // Check for invalid path patterns (Windows drive letters, etc.)
    // Allow relative paths (starting with .), absolute paths, and normal paths
    // Exclude: < > : " | ? * and control characters (\x00-\x1f)
    // Use a simpler validation: just check for null bytes and control characters
    // Paths can contain most characters except control chars
    for ch in path_trimmed.chars() {
        if ch.is_control() {
            return Err(anyhow::anyhow!(
                "{field_name} '{path_trimmed}' contains control characters"
            ));
        }
    }

    Ok(())
}

/// Validate provider configuration
/// Uses official provider API constraints from:
/// - GCP: https://cloud.google.com/resource-manager/docs/creating-managing-projects
/// - AWS: https://docs.aws.amazon.com/general/latest/gr/rande.html
/// - Azure: https://learn.microsoft.com/en-us/azure/key-vault/general/about-keys-secrets-certificates#vault-name
fn validate_provider_config(provider: &ProviderConfig) -> Result<()> {
    match provider {
        ProviderConfig::Gcp(gcp) => {
            if gcp.project_id.is_empty() {
                return Err(anyhow::anyhow!(
                    "provider.gcp.projectId is required but is empty"
                ));
            }
            // GCP project ID validation per official GCP API constraints:
            // - Length: 6-30 characters
            // - Must start with a lowercase letter
            // - Cannot end with a hyphen
            // - Allowed: lowercase letters, numbers, hyphens
            // Reference: https://cloud.google.com/resource-manager/docs/creating-managing-projects
            let project_id_regex = Regex::new(r"^[a-z][a-z0-9-]{4,28}[a-z0-9]$")
                .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

            if !project_id_regex.is_match(&gcp.project_id) {
                return Err(anyhow::anyhow!(
                    "provider.gcp.projectId '{}' must be a valid GCP project ID (6-30 characters, lowercase letters/numbers/hyphens, must start with letter, cannot end with hyphen). See: https://cloud.google.com/resource-manager/docs/creating-managing-projects",
                    gcp.project_id
                ));
            }
        }
        ProviderConfig::Aws(aws) => {
            if aws.region.is_empty() {
                return Err(anyhow::anyhow!(
                    "provider.aws.region is required but is empty"
                ));
            }
            // AWS region validation per official AWS API constraints:
            // - Format: [a-z]{2}-[a-z]+-[0-9]+ (e.g., us-east-1, eu-west-1)
            // - Some regions include -gov or -iso segments (e.g., us-gov-west-1)
            // - Must match valid AWS region codes
            // Reference: https://docs.aws.amazon.com/general/latest/gr/rande.html
            validate_aws_region(&aws.region)?;
        }
        ProviderConfig::Azure(azure) => {
            if azure.vault_name.is_empty() {
                return Err(anyhow::anyhow!(
                    "provider.azure.vaultName is required but is empty"
                ));
            }
            // Azure Key Vault name validation per official Azure API constraints:
            // - Length: 3-24 characters
            // - Must start with a letter
            // - Cannot end with a hyphen
            // - Allowed: alphanumeric characters and hyphens
            // - Hyphens cannot be consecutive
            // Reference: https://learn.microsoft.com/en-us/azure/key-vault/general/about-keys-secrets-certificates#vault-name
            let vault_name_regex = Regex::new(r"^[a-zA-Z][a-zA-Z0-9-]{1,22}[a-zA-Z0-9]$")
                .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

            if !vault_name_regex.is_match(&azure.vault_name) {
                return Err(anyhow::anyhow!(
                    "provider.azure.vaultName '{}' must be a valid Azure Key Vault name (3-24 characters, alphanumeric/hyphens, must start with letter, cannot end with hyphen). See: https://learn.microsoft.com/en-us/azure/key-vault/general/about-keys-secrets-certificates#vault-name",
                    azure.vault_name
                ));
            }

            // Check for consecutive hyphens
            if azure.vault_name.contains("--") {
                return Err(anyhow::anyhow!(
                    "provider.azure.vaultName '{}' cannot contain consecutive hyphens",
                    azure.vault_name
                ));
            }
        }
    }
    Ok(())
}

/// Validate AWS region against official AWS region format
/// Supports standard regions (us-east-1) and special regions (us-gov-west-1, cn-north-1)
/// Reference: https://docs.aws.amazon.com/general/latest/gr/rande.html
fn validate_aws_region(region: &str) -> Result<()> {
    let region_trimmed = region.trim().to_lowercase();

    if region_trimmed.is_empty() {
        return Err(anyhow::anyhow!("provider.aws.region cannot be empty"));
    }

    // AWS region format patterns:
    // Standard: [a-z]{2}-[a-z]+-[0-9]+ (e.g., us-east-1, eu-west-1)
    // Gov: [a-z]{2}-gov-[a-z]+-[0-9]+ (e.g., us-gov-west-1)
    // ISO: [a-z]{2}-iso-[a-z]+-[0-9]+ (e.g., us-iso-east-1)
    // China: cn-[a-z]+-[0-9]+ (e.g., cn-north-1)
    // Local: local (for localstack)

    // Standard region pattern: [a-z]{2}-[a-z]+-[0-9]+
    let standard_pattern = Regex::new(r"^[a-z]{2}-[a-z]+-\d+$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    // Gov region pattern: [a-z]{2}-gov-[a-z]+-[0-9]+
    let gov_pattern = Regex::new(r"^[a-z]{2}-gov-[a-z]+-\d+$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    // ISO region pattern: [a-z]{2}-iso-[a-z]+-[0-9]+
    let iso_pattern = Regex::new(r"^[a-z]{2}-iso-[a-z]+-\d+$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    // China region pattern: cn-[a-z]+-[0-9]+
    let china_pattern = Regex::new(r"^cn-[a-z]+-\d+$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    // Local pattern (for local development/testing with localstack)
    // Note: This allows "local" as a region for local development environments
    let local_pattern =
        Regex::new(r"^local$").map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if standard_pattern.is_match(&region_trimmed)
        || gov_pattern.is_match(&region_trimmed)
        || iso_pattern.is_match(&region_trimmed)
        || china_pattern.is_match(&region_trimmed)
        || local_pattern.is_match(&region_trimmed)
    {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "provider.aws.region '{region}' must be a valid AWS region code (e.g., 'us-east-1', 'eu-west-1', 'us-gov-west-1', 'cn-north-1'). See: https://docs.aws.amazon.com/general/latest/gr/rande.html"
        ))
    }
}

/// Validate configs configuration
fn validate_configs_config(configs: &crate::crd::ConfigsConfig) -> Result<()> {
    // Validate store type if present
    // ConfigStoreType is an enum, so it's already validated by serde
    // No additional validation needed - enum variants are: SecretManager, ParameterManager
    if let Some(ref _store) = configs.store {
        // Enum is already validated by serde deserialization
        // ConfigStoreType::SecretManager or ConfigStoreType::ParameterManager are the only valid values
    }

    // Validate appConfigEndpoint if present
    if let Some(endpoint) = &configs.app_config_endpoint {
        if !endpoint.is_empty() {
            if let Err(e) = validate_url(endpoint, "configs.appConfigEndpoint") {
                return Err(anyhow::anyhow!(
                    "Invalid configs.appConfigEndpoint '{}': {}",
                    endpoint,
                    e
                ));
            }
        }
    }

    // Validate parameterPath if present
    if let Some(path) = &configs.parameter_path {
        if !path.is_empty() {
            if let Err(e) = validate_aws_parameter_path(path, "configs.parameterPath") {
                return Err(anyhow::anyhow!(
                    "Invalid configs.parameterPath '{}': {}",
                    path,
                    e
                ));
            }
        }
    }

    Ok(())
}

/// Validate URL format
fn validate_url(url: &str, field_name: &str) -> Result<()> {
    let url_trimmed = url.trim();

    if url_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    // Basic URL validation: must start with http:// or https://
    let url_regex = Regex::new(r"^https?://[^\s/$.?#].[^\s]*$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !url_regex.is_match(url_trimmed) {
        return Err(anyhow::anyhow!(
            "{field_name} '{url_trimmed}' must be a valid URL starting with http:// or https://"
        ));
    }

    Ok(())
}

/// Validate AWS Parameter Store path
/// Format: /path/to/parameter (must start with /)
fn validate_aws_parameter_path(path: &str, field_name: &str) -> Result<()> {
    let path_trimmed = path.trim();

    if path_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    if !path_trimmed.starts_with('/') {
        return Err(anyhow::anyhow!(
            "{field_name} '{path_trimmed}' must start with '/' (e.g., '/my-service/dev')"
        ));
    }

    // AWS Parameter Store path: /[a-zA-Z0-9._-]+(/[a-zA-Z0-9._-]+)*
    let param_path_regex = Regex::new(r"^/[a-zA-Z0-9._-]+(/[a-zA-Z0-9._-]+)*$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !param_path_regex.is_match(path_trimmed) {
        return Err(anyhow::anyhow!(
            "{field_name} '{path_trimmed}' must be a valid AWS Parameter Store path (e.g., '/my-service/dev')"
        ));
    }

    Ok(())
}
