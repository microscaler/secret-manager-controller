//! Pact contract tests for Azure Key Vault Secrets API
//!
//! These tests define the contract between the Secret Manager Controller and Azure Key Vault Secrets API.
//! They use Pact to create a mock server that simulates Azure Key Vault responses.

use pact_consumer::prelude::*;
use serde_json::json;
use reqwest;

// Helper function to make HTTP requests to the mock server
async fn make_request(
    client: &reqwest::Client,
    method: &str,
    url: &str,
    body: Option<serde_json::Value>,
    query_params: Option<Vec<(&str, &str)>>,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut request = match method {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        _ => panic!("Unsupported HTTP method: {}", method),
    };

    request = request
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json");

    if let Some(params) = query_params {
        for (key, value) in params {
            request = request.query(&[(key, value)]);
        }
    }

    if let Some(body) = body {
        request = request.json(&body);
    }

    request.send().await
}

#[tokio::test]
async fn test_azure_set_secret_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-Key-Vault");

    pact_builder.interaction("set a secret in Azure Key Vault", "", |mut i| {
        i.given("Azure Key Vault exists and credentials are configured");
        i.request
            .method("PUT")
            .path("/secrets/test-secret-name")
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .query_param("api-version", "7.4")
            .json_body(json!({
                "value": "test-secret-value"
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "value": "test-secret-value",
                "id": "https://test-vault.vault.azure.net/secrets/test-secret-name/abc123",
                "attributes": {
                    "enabled": true,
                    "created": 1704067200,
                    "updated": 1704067200,
                    "recoveryLevel": "Recoverable+Purgeable"
                }
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    // mock_server.url() returns a Url struct - convert to string and strip trailing slash
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    // Path always starts with /
    let mock_url = format!("{}/secrets/test-secret-name", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "PUT",
        &mock_url,
        Some(json!({
            "value": "test-secret-value"
        })),
        Some(vec![("api-version", "7.4")]),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["value"], "test-secret-value");
}

#[tokio::test]
async fn test_azure_get_secret_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-Key-Vault");

    pact_builder.interaction("get the latest version of a secret", "", |mut i| {
        i.given("a secret exists in Azure Key Vault");
        i.request
            .method("GET")
            .path("/secrets/test-secret-name")
            .header("authorization", "Bearer test-token")
            .query_param("api-version", "7.4");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "value": "test-secret-value",
                "id": "https://test-vault.vault.azure.net/secrets/test-secret-name/abc123",
                "attributes": {
                    "enabled": true,
                    "created": 1704067200,
                    "updated": 1704067200,
                    "recoveryLevel": "Recoverable+Purgeable"
                }
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    // mock_server.url() returns a Url struct - convert to string and strip trailing slash
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    // Path always starts with /
    let mock_url = format!("{}/secrets/test-secret-name", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "GET",
        &mock_url,
        None,
        Some(vec![("api-version", "7.4")]),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["value"], "test-secret-value");
}

#[tokio::test]
async fn test_azure_get_secret_version_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-Key-Vault");

    pact_builder.interaction("get a specific version of a secret", "", |mut i| {
        i.given("a secret exists with multiple versions");
        i.request
            .method("GET")
            .path("/secrets/test-secret-name/abc123")
            .header("authorization", "Bearer test-token")
            .query_param("api-version", "7.4");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "value": "test-secret-value",
                "id": "https://test-vault.vault.azure.net/secrets/test-secret-name/abc123",
                "attributes": {
                    "enabled": true,
                    "created": 1704067200,
                    "updated": 1704067200,
                    "recoveryLevel": "Recoverable+Purgeable"
                }
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    // mock_server.url() returns a Url struct - convert to string and strip trailing slash
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    // Path always starts with /
    let mock_url = format!("{}/secrets/test-secret-name/abc123", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "GET",
        &mock_url,
        None,
        Some(vec![("api-version", "7.4")]),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["value"], "test-secret-value");
}

#[tokio::test]
async fn test_azure_delete_secret_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-Key-Vault");

    pact_builder.interaction("delete a secret", "", |mut i| {
        i.given("a secret exists in Azure Key Vault");
        i.request
            .method("DELETE")
            .path("/secrets/test-secret-name")
            .header("authorization", "Bearer test-token")
            .query_param("api-version", "7.4");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "recoveryId": "https://test-vault.vault.azure.net/deletedsecrets/test-secret-name",
                "deletedDate": 1704067200,
                "scheduledPurgeDate": 1704672000,
                "recoveryLevel": "Recoverable+Purgeable"
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    // mock_server.url() returns a Url struct - convert to string and strip trailing slash
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    // Path always starts with /
    let mock_url = format!("{}/secrets/test-secret-name", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "DELETE",
        &mock_url,
        None,
        Some(vec![("api-version", "7.4")]),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["recoveryId"], "https://test-vault.vault.azure.net/deletedsecrets/test-secret-name");
}

#[tokio::test]
async fn test_azure_secret_not_found_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-Key-Vault");

    pact_builder
        .interaction("get a secret that does not exist", "", |mut i| {
            i.given("the secret does not exist");
            i.request
                .method("GET")
                .path("/secrets/non-existent-secret")
                .header("authorization", "Bearer test-token")
                .query_param("api-version", "7.4");
            i.response
                .status(404)
                .header("content-type", "application/json")
                .json_body(json!({
                    "error": {
                        "code": "SecretNotFound",
                        "message": "A secret with (name/id) non-existent-secret was not found in this key vault."
                    }
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    // mock_server.url() returns a Url struct - convert to string and strip trailing slash
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    // Path always starts with /
    let mock_url = format!("{}/secrets/non-existent-secret", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "GET",
        &mock_url,
        None,
        Some(vec![("api-version", "7.4")]),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 404);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["error"]["code"], "SecretNotFound");
}
