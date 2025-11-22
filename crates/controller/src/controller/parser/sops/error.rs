//! # SOPS Decryption Error Types
//!
//! Defines error types for SOPS decryption with classification of transient vs permanent failures.

use thiserror::Error;

/// SOPS decryption error with classification
#[derive(Debug, Error)]
#[error("SOPS decryption failed: {reason:?} - {message}")]
pub struct SopsDecryptionError {
    pub reason: SopsDecryptionFailureReason,
    pub message: String,
    pub is_transient: bool,
}

impl SopsDecryptionError {
    pub fn new(reason: SopsDecryptionFailureReason, message: String) -> Self {
        let is_transient = reason.is_transient();
        Self {
            reason,
            message,
            is_transient,
        }
    }

    /// Get remediation guidance for this error
    pub fn remediation(&self) -> String {
        self.reason.remediation()
    }
}

/// Classification of SOPS decryption failure reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SopsDecryptionFailureReason {
    /// Key not found in namespace (permanent - configuration error)
    KeyNotFound,
    /// Wrong key provided (permanent - configuration error)
    WrongKey,
    /// Key format invalid (permanent - configuration error)
    InvalidKeyFormat,
    /// File format unsupported (permanent - file issue)
    UnsupportedFormat,
    /// File corrupted or invalid (permanent - file issue)
    CorruptedFile,
    /// Network timeout contacting GPG/key provider (transient)
    NetworkTimeout,
    /// GPG/key provider unavailable (transient)
    ProviderUnavailable,
    /// RBAC/permission denied (may be transient if RBAC is being applied)
    PermissionDenied,
    /// Unknown error (assume transient for safety)
    Unknown,
}

impl SopsDecryptionFailureReason {
    /// Determine if this error is transient (should retry) or permanent (should fail immediately)
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            SopsDecryptionFailureReason::NetworkTimeout
                | SopsDecryptionFailureReason::ProviderUnavailable
                | SopsDecryptionFailureReason::PermissionDenied
                | SopsDecryptionFailureReason::Unknown
        )
    }

    /// Get human-readable reason string for metrics
    pub fn as_str(&self) -> &'static str {
        match self {
            SopsDecryptionFailureReason::KeyNotFound => "key_not_found",
            SopsDecryptionFailureReason::WrongKey => "wrong_key",
            SopsDecryptionFailureReason::InvalidKeyFormat => "invalid_key_format",
            SopsDecryptionFailureReason::UnsupportedFormat => "unsupported_format",
            SopsDecryptionFailureReason::CorruptedFile => "corrupted_file",
            SopsDecryptionFailureReason::NetworkTimeout => "network_timeout",
            SopsDecryptionFailureReason::ProviderUnavailable => "provider_unavailable",
            SopsDecryptionFailureReason::PermissionDenied => "permission_denied",
            SopsDecryptionFailureReason::Unknown => "unknown",
        }
    }

    /// Get remediation guidance for this error type
    pub fn remediation(&self) -> String {
        match self {
            SopsDecryptionFailureReason::KeyNotFound => {
                "Create SOPS private key secret in the resource namespace. Expected secret names: 'sops-private-key', 'sops-gpg-key', or 'gpg-key' with key 'private-key', 'key', or 'gpg-key'.".to_string()
            }
            SopsDecryptionFailureReason::WrongKey => {
                "Verify the SOPS private key matches the encryption key used in .sops.yaml. Check the key fingerprint in the file metadata.".to_string()
            }
            SopsDecryptionFailureReason::InvalidKeyFormat => {
                "Ensure the SOPS private key is in ASCII-armored GPG format (-----BEGIN PGP PRIVATE KEY BLOCK-----...-----END PGP PRIVATE KEY BLOCK-----)".to_string()
            }
            SopsDecryptionFailureReason::UnsupportedFormat => {
                "SOPS file format is not supported. Supported formats: dotenv (.env), YAML (.yaml/.yml), JSON (.json)".to_string()
            }
            SopsDecryptionFailureReason::CorruptedFile => {
                "SOPS file appears corrupted. Verify the file was encrypted correctly and hasn't been modified.".to_string()
            }
            SopsDecryptionFailureReason::NetworkTimeout => {
                "Network timeout contacting GPG/key provider. This is usually transient - will retry.".to_string()
            }
            SopsDecryptionFailureReason::ProviderUnavailable => {
                "GPG/key provider is unavailable. Check if GPG service is running and accessible. This is usually transient - will retry.".to_string()
            }
            SopsDecryptionFailureReason::PermissionDenied => {
                "Permission denied accessing SOPS key secret. Verify RBAC is configured correctly and ServiceAccount has 'get' permission for the secret.".to_string()
            }
            SopsDecryptionFailureReason::Unknown => {
                "Unknown SOPS decryption error. Check controller logs for detailed error message.".to_string()
            }
        }
    }
}

/// Classify SOPS decryption error from error message and exit code
///
/// SOPS exit codes (from SOPS source code):
/// - 1: General error
/// - 2: File not found
/// - 3: Key not found (no decryption key available)
/// - 4: Wrong key (key doesn't match encryption key)
/// - 5: Invalid file format
/// - 6: Invalid key format
///
/// Exit codes are more reliable than error message parsing, so we check them first.
/// If exit code is not available or doesn't provide clear classification, we fall back
/// to parsing the error message.
pub fn classify_sops_error(error_msg: &str, exit_code: Option<i32>) -> SopsDecryptionFailureReason {
    // First, check exit code for reliable classification
    // SOPS exit codes are more reliable than error message parsing
    if let Some(code) = exit_code {
        match code {
            3 => return SopsDecryptionFailureReason::KeyNotFound,
            4 => return SopsDecryptionFailureReason::WrongKey,
            5 => return SopsDecryptionFailureReason::UnsupportedFormat,
            6 => return SopsDecryptionFailureReason::InvalidKeyFormat,
            2 => return SopsDecryptionFailureReason::CorruptedFile, // File not found often means corrupted path or invalid file
            // Exit code 1 is generic, so we fall through to error message parsing
            _ => {
                // For other exit codes, fall through to error message parsing
            }
        }
    }

    // Fall back to error message parsing if exit code doesn't provide clear classification
    let error_lower = error_msg.to_lowercase();

    // Check for permanent failures first
    if error_lower.contains("no decryption key") || error_lower.contains("key not found") {
        return SopsDecryptionFailureReason::KeyNotFound;
    }

    if error_lower.contains("wrong key") || error_lower.contains("decryption failed") {
        if error_lower.contains("gpg") || error_lower.contains("key") {
            return SopsDecryptionFailureReason::WrongKey;
        }
    }

    if error_lower.contains("invalid key") || error_lower.contains("malformed key") {
        return SopsDecryptionFailureReason::InvalidKeyFormat;
    }

    if error_lower.contains("unsupported format") || error_lower.contains("unknown file type") {
        return SopsDecryptionFailureReason::UnsupportedFormat;
    }

    if error_lower.contains("corrupt") || error_lower.contains("invalid file") {
        return SopsDecryptionFailureReason::CorruptedFile;
    }

    // Check for transient failures
    if error_lower.contains("timeout") || error_lower.contains("timed out") {
        return SopsDecryptionFailureReason::NetworkTimeout;
    }

    if error_lower.contains("unavailable") || error_lower.contains("connection refused") {
        return SopsDecryptionFailureReason::ProviderUnavailable;
    }

    if error_lower.contains("permission denied")
        || error_lower.contains("unauthorized")
        || error_lower.contains("forbidden")
    {
        return SopsDecryptionFailureReason::PermissionDenied;
    }

    // Default to unknown (treated as transient for safety)
    SopsDecryptionFailureReason::Unknown
}
