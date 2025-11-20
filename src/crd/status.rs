//! # SecretManagerConfig Status
//!
//! Status types for tracking reconciliation state and conditions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
