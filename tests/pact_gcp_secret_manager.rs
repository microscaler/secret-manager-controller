//! Pact contract tests for GCP Secret Manager API
//!
//! These tests define the contract between the Secret Manager Controller and GCP Secret Manager API.
//! They use Pact to create a mock server that simulates GCP Secret Manager responses.

use pact_consumer::prelude::*;
use serde_json::json;

// Helper function to make HTTP requests to the mock server
async fn make_request(
    client: &reqwest::Client,
    method: &str,
    url: &str,
    body: Option<serde_json::Value>,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut request = match method {
        "GET" => client.get(url),
        "POST" => client.post(url),
        _ => panic!("Unsupported HTTP method: {}", method),
    };

    request = request.header("authorization", "Bearer test-token");

    if let Some(body) = body {
        request = request.json(&body);
    }

    request.send().await
}

#[tokio::test]
async fn test_gcp_create_secret_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    pact_builder.interaction("create a new secret in GCP Secret Manager", "", |mut i| {
        i.given("a GCP project exists");
        i.request
            .method("POST")
            .path("/v1/projects/test-project/secrets".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .json_body(json!({
                "secretId": "test-secret-name",
                "replication": {
                    "automatic": {}
                }
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/secrets/test-secret-name",
                "replication": {
                    "automatic": {}
                },
                "createTime": "2024-01-01T00:00:00Z"
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
    let mock_url = format!("{}/v1/projects/test-project/secrets", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "secretId": "test-secret-name",
            "replication": {
                "automatic": {}
            }
        })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["name"], "projects/test-project/secrets/test-secret-name");
}

#[tokio::test]
async fn test_gcp_add_secret_version_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    pact_builder.interaction("add a secret version to an existing secret", "", |mut i| {
        i.given("a secret exists in GCP Secret Manager");
        i.request
            .method("POST")
            .path("/v1/projects/test-project/secrets/test-secret-name:addVersion".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .json_body(json!({
                "payload": {
                    "data": "dGVzdC1zZWNyZXQtdmFsdWU="
                }
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/secrets/test-secret-name/versions/1",
                "payload": {
                    "data": "dGVzdC1zZWNyZXQtdmFsdWU="
                },
                "createTime": "2024-01-01T00:00:00Z",
                "state": "ENABLED"
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
    let mock_url = format!("{}/v1/projects/test-project/secrets/test-secret-name:addVersion", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "payload": {
                "data": "dGVzdC1zZWNyZXQtdmFsdWU="
            }
        })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["name"], "projects/test-project/secrets/test-secret-name/versions/1");
}

#[tokio::test]
async fn test_gcp_get_secret_version_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    pact_builder.interaction("get the latest version of a secret", "", |mut i| {
        i.given("a secret exists with at least one version");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/secrets/test-secret-name/versions/latest".to_string())
            .header("authorization", "Bearer test-token");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/secrets/test-secret-name/versions/1",
                "payload": {
                    "data": "dGVzdC1zZWNyZXQtdmFsdWU="
                },
                "createTime": "2024-01-01T00:00:00Z",
                "state": "ENABLED"
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
    let mock_url = format!("{}/v1/projects/test-project/secrets/test-secret-name/versions/latest", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "GET",
        &mock_url,
        None,
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["name"], "projects/test-project/secrets/test-secret-name/versions/1");
}

#[tokio::test]
async fn test_gcp_secret_not_found_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    pact_builder.interaction("get a secret that does not exist", "", |mut i| {
        i.given("the secret does not exist");
        i.request
            .method("GET")
            .path(
                "/v1/projects/test-project/secrets/non-existent-secret/versions/latest".to_string(),
            )
            .header("authorization", "Bearer test-token");
        i.response
            .status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": {
                    "code": 404,
                    "message": "Secret [non-existent-secret] not found",
                    "status": "NOT_FOUND"
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
    let mock_url = format!("{}/v1/projects/test-project/secrets/non-existent-secret/versions/latest", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "GET",
        &mock_url,
        None,
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 404);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["error"]["code"], 404);
}
