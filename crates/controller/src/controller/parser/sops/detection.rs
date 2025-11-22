//! # SOPS Encryption Detection
//!
//! Detects if content is SOPS-encrypted by looking for SOPS metadata.

/// Check if content is SOPS-encrypted by looking for SOPS metadata
/// Public for integration tests
pub fn is_sops_encrypted(content: &str) -> bool {
    is_sops_encrypted_impl(content)
}

/// Internal implementation of SOPS encryption detection
/// Public for internal use and tests
pub fn is_sops_encrypted_impl(content: &str) -> bool {
    // SOPS files have a specific structure with sops metadata
    // Check for common SOPS indicators:
    // 1. YAML files start with "sops:" key
    // 2. JSON files have "sops" key at root
    // 3. ENV files might have SOPS metadata comments

    // Try parsing as YAML first (most common)
    if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(content) {
        if yaml
            .as_mapping()
            .and_then(|m| m.get(serde_yaml::Value::String("sops".to_string())))
            .is_some()
        {
            return true;
        }
    }

    // Try parsing as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        if json.get("sops").is_some() {
            return true;
        }
    }

    // Check for SOPS metadata in comments (for ENV files)
    if content.contains("sops_version") || content.contains("sops_encrypted") {
        return true;
    }

    // Check for ENC[...] patterns (SOPS encrypted values in dotenv files)
    // Pattern: ENC[AES256_GCM,data:...,iv:...,tag:...,type:...]
    if content.contains("ENC[") && content.contains("AES256_GCM") {
        return true;
    }

    false
}
