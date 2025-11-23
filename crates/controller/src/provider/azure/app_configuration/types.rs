//! # Azure App Configuration Types
//!
//! Request and response types for Azure App Configuration API.

use serde::{Deserialize, Serialize};

/// Key-value pair for Azure App Configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}
