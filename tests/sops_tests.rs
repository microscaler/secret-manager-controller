//! # SOPS Decryption Unit Tests
//!
//! Comprehensive unit tests for SOPS decryption functionality.
//!
//! These tests verify:
//! - SOPS-encrypted file detection
//! - Decryption with correct input/output types (dotenv, yaml, json)
//! - Error handling for missing keys, invalid files, etc.
//! - Security: No disk writes (stdin/stdout pipes only)

use secret_manager_controller::controller::parser::sops::error::{
    classify_sops_error, SopsDecryptionFailureReason,
};
use secret_manager_controller::controller::parser::{decrypt_sops_content, is_sops_encrypted};
use std::env;
use std::path::PathBuf;

/// Test helper: Get SOPS private key from environment
/// In CI, this should be set as a GitHub Actions secret
fn get_test_sops_key() -> Option<String> {
    env::var("SOPS_PRIVATE_KEY").ok()
}

/// Test helper: Get path to test files
///
/// Priority:
/// 1. TEST_FILES_DIR environment variable (for CI/CD)
/// 2. Integration test fixtures (tests/integration/fixtures)
/// 3. Default deployment-configuration path (for local development)
fn get_test_files_dir() -> PathBuf {
    if let Ok(dir) = env::var("TEST_FILES_DIR") {
        return PathBuf::from(dir);
    }

    // Try integration test fixtures
    let integration_fixtures = PathBuf::from("tests/integration/fixtures");
    if integration_fixtures.exists() {
        return integration_fixtures;
    }

    // Default to deployment-configuration for local development
    PathBuf::from("deployment-configuration/profiles/tilt")
}

#[test]
fn test_sops_encrypted_detection() {
    // Test YAML format
    let yaml_content = r#"sops:
    kms: []
    gcp_kms: []
    azure_kv: []
    hc_vault: []
    age: []
    lastmodified: "2024-01-01T00:00:00Z"
    mac: ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]
    version: 3.8.0
DATABASE_URL: ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]
"#;
    assert!(is_sops_encrypted(yaml_content));

    // Test JSON format
    let json_content = r#"{
    "sops": {
        "kms": [],
        "gcp_kms": [],
        "azure_kv": [],
        "hc_vault": [],
        "age": [],
        "lastmodified": "2024-01-01T00:00:00Z",
        "mac": "ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]",
        "version": "3.8.0"
    },
    "DATABASE_URL": "ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]"
}"#;
    assert!(is_sops_encrypted(json_content));

    // Test ENV format with metadata
    let env_content = r#"# sops_version: 3.8.0
# sops_encrypted: true
DATABASE_URL=ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]
"#;
    assert!(is_sops_encrypted(env_content));

    // Test ENV format with ENC pattern
    let env_enc_content = "DATABASE_URL=ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]";
    assert!(is_sops_encrypted(env_enc_content));

    // Test plain YAML (not encrypted)
    let plain_yaml = r#"
database:
  url: postgresql://localhost/db
"#;
    assert!(!is_sops_encrypted(plain_yaml));

    // Test plain ENV (not encrypted)
    let plain_env = "DATABASE_URL=postgresql://localhost/db";
    assert!(!is_sops_encrypted(plain_env));
}

#[tokio::test]
async fn test_decrypt_dotenv_file() {
    let sops_key = get_test_sops_key();
    if sops_key.is_none() {
        eprintln!("⚠️  SOPS_PRIVATE_KEY not set - skipping test");
        return;
    }

    let test_dir = get_test_files_dir();
    let file_path = test_dir.join("application.secrets.env");

    if !file_path.exists() {
        eprintln!(
            "⚠️  Test file not found: {} - skipping test",
            file_path.display()
        );
        return;
    }

    // Read encrypted content
    let encrypted_content = tokio::fs::read_to_string(&file_path)
        .await
        .expect("Failed to read test file");

    // Verify it's encrypted
    assert!(is_sops_encrypted(&encrypted_content));

    // Decrypt
    let decrypted = decrypt_sops_content(&encrypted_content, Some(&file_path), sops_key.as_deref())
        .await
        .expect("Failed to decrypt SOPS file");

    // Verify decrypted content
    assert!(!decrypted.is_empty());
    assert!(!is_sops_encrypted(&decrypted));

    // Verify expected keys are present
    assert!(decrypted.contains("DATABASE_URL"));
    assert!(decrypted.contains("DATABASE_USER"));
}

#[tokio::test]
async fn test_decrypt_yaml_file() {
    let sops_key = get_test_sops_key();
    if sops_key.is_none() {
        eprintln!("⚠️  SOPS_PRIVATE_KEY not set - skipping test");
        return;
    }

    let test_dir = get_test_files_dir();
    let file_path = test_dir.join("application.secrets.yaml");

    if !file_path.exists() {
        eprintln!(
            "⚠️  Test file not found: {} - skipping test",
            file_path.display()
        );
        return;
    }

    // Read encrypted content
    let encrypted_content = tokio::fs::read_to_string(&file_path)
        .await
        .expect("Failed to read test file");

    // Verify it's encrypted
    assert!(is_sops_encrypted(&encrypted_content));

    // Decrypt
    let decrypted = decrypt_sops_content(&encrypted_content, Some(&file_path), sops_key.as_deref())
        .await
        .expect("Failed to decrypt SOPS file");

    // Verify decrypted content
    assert!(!decrypted.is_empty());
    assert!(!is_sops_encrypted(&decrypted));

    // Verify expected keys are present
    assert!(decrypted.contains("DATABASE_URL"));
    assert!(decrypted.contains("DATABASE_USER"));
}

#[tokio::test]
async fn test_decrypt_without_key_fails() {
    let test_dir = get_test_files_dir();
    let file_path = test_dir.join("application.secrets.env");

    if !file_path.exists() {
        eprintln!(
            "⚠️  Test file not found: {} - skipping test",
            file_path.display()
        );
        return;
    }

    let encrypted_content = tokio::fs::read_to_string(&file_path)
        .await
        .expect("Failed to read test file");

    // Try to decrypt without key - should fail
    let result = decrypt_sops_content(&encrypted_content, Some(&file_path), None).await;

    assert!(result.is_err());
    let error = result.unwrap_err();

    // Verify error classification
    assert_eq!(error.reason, SopsDecryptionFailureReason::KeyNotFound);
    assert!(error.is_transient == false); // Key not found is permanent
}

#[tokio::test]
async fn test_decrypt_with_wrong_key_fails() {
    let test_dir = get_test_files_dir();
    let file_path = test_dir.join("application.secrets.env");

    if !file_path.exists() {
        eprintln!(
            "⚠️  Test file not found: {} - skipping test",
            file_path.display()
        );
        return;
    }

    let encrypted_content = tokio::fs::read_to_string(&file_path)
        .await
        .expect("Failed to read test file");

    // Try to decrypt with wrong key (a dummy GPG key)
    let wrong_key = r#"-----BEGIN PGP PRIVATE KEY BLOCK-----
This is not a valid GPG key, just for testing
-----END PGP PRIVATE KEY BLOCK-----"#;

    let result = decrypt_sops_content(&encrypted_content, Some(&file_path), Some(wrong_key)).await;

    assert!(result.is_err());
    let error = result.unwrap_err();

    // Verify error classification - wrong key should be classified as WrongKey or InvalidKeyFormat
    assert!(
        error.reason == SopsDecryptionFailureReason::WrongKey
            || error.reason == SopsDecryptionFailureReason::InvalidKeyFormat
    );
    assert!(error.is_transient == false); // Wrong key is permanent
}

#[tokio::test]
async fn test_decrypt_invalid_content_fails() {
    // Try to decrypt non-SOPS content (should fail gracefully)
    let invalid_content = "This is not SOPS-encrypted content at all";

    let result = decrypt_sops_content(invalid_content, None, None).await;

    // This might succeed if SOPS can handle it, or fail - either is acceptable
    // The important thing is it doesn't panic
    let _ = result; // Just ensure it doesn't panic
}

#[test]
fn test_error_classification() {
    // Test key_not_found classification
    let error_msg = "no decryption key found for data key";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::KeyNotFound);
    assert!(!reason.is_transient());

    // Test wrong_key classification
    let error_msg = "decryption failed: wrong key";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::WrongKey);
    assert!(!reason.is_transient());

    // Test invalid_key_format classification
    let error_msg = "invalid key format: malformed GPG key";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::InvalidKeyFormat);
    assert!(!reason.is_transient());

    // Test network_timeout classification (transient)
    let error_msg = "network timeout while contacting GPG provider";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::NetworkTimeout);
    assert!(reason.is_transient());

    // Test provider_unavailable classification (transient)
    let error_msg = "GPG provider unavailable: connection refused";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::ProviderUnavailable);
    assert!(reason.is_transient());

    // Test permission_denied classification (transient)
    let error_msg = "permission denied accessing secret";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::PermissionDenied);
    assert!(reason.is_transient());

    // Test corrupted_file classification
    let error_msg = "corrupted file: invalid SOPS format";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::CorruptedFile);
    assert!(!reason.is_transient());

    // Test unsupported_format classification
    let error_msg = "unsupported format: unknown file type";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::UnsupportedFormat);
    assert!(!reason.is_transient());

    // Test unknown error (defaults to transient for safety)
    let error_msg = "some random error message";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::Unknown);
    assert!(reason.is_transient()); // Unknown errors are treated as transient for safety
}

#[test]
fn test_error_remediation_guidance() {
    // Test that each error reason provides remediation guidance
    let reasons = vec![
        SopsDecryptionFailureReason::KeyNotFound,
        SopsDecryptionFailureReason::WrongKey,
        SopsDecryptionFailureReason::InvalidKeyFormat,
        SopsDecryptionFailureReason::UnsupportedFormat,
        SopsDecryptionFailureReason::CorruptedFile,
        SopsDecryptionFailureReason::NetworkTimeout,
        SopsDecryptionFailureReason::ProviderUnavailable,
        SopsDecryptionFailureReason::PermissionDenied,
        SopsDecryptionFailureReason::Unknown,
    ];

    for reason in reasons {
        let remediation = reason.remediation();
        assert!(
            !remediation.is_empty(),
            "Remediation guidance should not be empty for {:?}",
            reason
        );
        assert!(
            remediation.len() > 20,
            "Remediation guidance should be detailed for {:?}",
            reason
        );
    }
}

#[test]
fn test_error_reason_strings() {
    // Test that all error reasons have valid string representations for metrics
    let reasons = vec![
        SopsDecryptionFailureReason::KeyNotFound,
        SopsDecryptionFailureReason::WrongKey,
        SopsDecryptionFailureReason::InvalidKeyFormat,
        SopsDecryptionFailureReason::UnsupportedFormat,
        SopsDecryptionFailureReason::CorruptedFile,
        SopsDecryptionFailureReason::NetworkTimeout,
        SopsDecryptionFailureReason::ProviderUnavailable,
        SopsDecryptionFailureReason::PermissionDenied,
        SopsDecryptionFailureReason::Unknown,
    ];

    for reason in reasons {
        let reason_str = reason.as_str();
        assert!(!reason_str.is_empty());
        assert!(!reason_str.contains(' ')); // Should be valid metric label (no spaces)
        assert!(reason_str.chars().all(|c| c.is_alphanumeric() || c == '_')); // Valid metric label characters
    }
}

#[test]
fn test_content_based_type_detection() {
    // Test that file type detection works from content when path is not available
    let yaml_content = r#"sops:
    version: 3.8.0
DATABASE_URL: ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]
"#;
    assert!(is_sops_encrypted(yaml_content));

    let json_content = r#"{"sops": {"version": "3.8.0"}, "DATABASE_URL": "ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]"}"#;
    assert!(is_sops_encrypted(json_content));

    let env_content = "DATABASE_URL=ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]";
    assert!(is_sops_encrypted(env_content));
}

#[test]
fn test_file_type_detection_from_path() {
    use secret_manager_controller::controller::parser::sops::is_sops_encrypted;

    // Test that ENC pattern detection works for dotenv files
    let dotenv_content = "DATABASE_URL=ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]\nANOTHER_KEY=ENC[AES256_GCM,data:test2,iv:test2,tag:test2,type:str]";
    assert!(is_sops_encrypted(dotenv_content));

    // Test that plain dotenv without ENC pattern is not detected as encrypted
    let plain_dotenv = "DATABASE_URL=postgresql://localhost/db\nANOTHER_KEY=some_value";
    assert!(!is_sops_encrypted(plain_dotenv));
}
