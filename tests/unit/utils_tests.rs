//! # Utility Functions Unit Tests
//!
//! Comprehensive unit tests for utility functions used throughout the reconciler.
//!
//! These tests verify:
//! - Secret name construction
//! - Secret name sanitization
//! - Path component sanitization

use controller::controller::reconciler::utils::{
    construct_secret_name, sanitize_path_component, sanitize_secret_name,
};

#[test]
fn test_construct_secret_name_with_all_components() {
    let result = construct_secret_name(Some("prefix"), "key", Some("suffix"));
    assert_eq!(result, "prefix-key-suffix");
}

#[test]
fn test_construct_secret_name_with_prefix_only() {
    let result = construct_secret_name(Some("prefix"), "key", None);
    assert_eq!(result, "prefix-key");
}

#[test]
fn test_construct_secret_name_with_suffix_only() {
    let result = construct_secret_name(None, "key", Some("suffix"));
    assert_eq!(result, "key-suffix");
}

#[test]
fn test_construct_secret_name_no_prefix_no_suffix() {
    let result = construct_secret_name(None, "key", None);
    assert_eq!(result, "key");
}

#[test]
fn test_construct_secret_name_empty_components() {
    // Empty prefix/suffix should be treated as None
    let result1 = construct_secret_name(Some(""), "key", None);
    assert_eq!(result1, "key");

    let result2 = construct_secret_name(None, "key", Some(""));
    assert_eq!(result2, "key");

    let result3 = construct_secret_name(Some(""), "key", Some(""));
    assert_eq!(result3, "key");
}

#[test]
fn test_construct_secret_name_multiple_segments() {
    let result = construct_secret_name(Some("my-service"), "database-password", Some("prod"));
    assert_eq!(result, "my-service-database-password-prod");
}

#[test]
fn test_sanitize_secret_name_dots() {
    assert_eq!(sanitize_secret_name("test.key"), "test_key");
    assert_eq!(sanitize_secret_name("test.key.value"), "test_key_value");
}

#[test]
fn test_sanitize_secret_name_slashes() {
    assert_eq!(sanitize_secret_name("test/key"), "test_key");
    assert_eq!(sanitize_secret_name("test/key/value"), "test_key_value");
}

#[test]
fn test_sanitize_secret_name_spaces() {
    assert_eq!(sanitize_secret_name("test key"), "test_key");
    assert_eq!(sanitize_secret_name("test key value"), "test_key_value");
}

#[test]
fn test_sanitize_secret_name_consecutive_dashes() {
    assert_eq!(sanitize_secret_name("test--key"), "test-key");
    assert_eq!(sanitize_secret_name("test---key"), "test-key");
}

#[test]
fn test_sanitize_secret_name_leading_trailing_dashes() {
    assert_eq!(sanitize_secret_name("--test--"), "test");
    assert_eq!(sanitize_secret_name("-test-"), "test");
    assert_eq!(sanitize_secret_name("---test---"), "test");
}

#[test]
fn test_sanitize_secret_name_valid_chars() {
    assert_eq!(sanitize_secret_name("test-key_123"), "test-key_123");
    assert_eq!(sanitize_secret_name("test123"), "test123");
    assert_eq!(sanitize_secret_name("test_key"), "test_key");
}

#[test]
fn test_sanitize_secret_name_mixed_invalid_chars() {
    assert_eq!(sanitize_secret_name("test.key/value name"), "test_key_value_name");
    assert_eq!(sanitize_secret_name("test--key.value/name"), "test-key_value_name");
}

#[test]
fn test_sanitize_secret_name_empty() {
    assert_eq!(sanitize_secret_name(""), "");
}

#[test]
fn test_sanitize_secret_name_only_invalid_chars() {
    assert_eq!(sanitize_secret_name("..."), "");
    assert_eq!(sanitize_secret_name("///"), "");
    assert_eq!(sanitize_secret_name("---"), "");
}

#[test]
fn test_sanitize_path_component_dots() {
    assert_eq!(sanitize_path_component("test.key"), "test_key");
}

#[test]
fn test_sanitize_path_component_slashes() {
    assert_eq!(sanitize_path_component("test/key"), "test_key");
}

#[test]
fn test_sanitize_path_component_spaces() {
    assert_eq!(sanitize_path_component("test key"), "test_key");
}

#[test]
fn test_sanitize_path_component_valid_chars() {
    assert_eq!(sanitize_path_component("test-key_123"), "test-key_123");
}

#[test]
fn test_sanitize_path_component_empty() {
    assert_eq!(sanitize_path_component(""), "");
}

#[test]
fn test_sanitize_path_component_special_chars() {
    // Path components should sanitize various special characters
    assert_eq!(sanitize_path_component("test@key"), "test_key");
    assert_eq!(sanitize_path_component("test#key"), "test_key");
    assert_eq!(sanitize_path_component("test$key"), "test_key");
}

