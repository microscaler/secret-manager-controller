//! GCP Secret Manager Mock Server
//!
//! A lightweight Axum-based HTTP server that serves as a mock for the GCP Secret Manager REST API.
//! Loads contracts from the Pact broker and serves them as mock responses.
//!
//! Environment Variables:
//! - PACT_BROKER_URL: URL of the Pact broker (default: http://pact-broker:9292)
//! - PACT_BROKER_USERNAME: Username for broker authentication (default: pact)
//! - PACT_BROKER_PASSWORD: Password for broker authentication (default: pact)
//! - PACT_PROVIDER: Provider name in contracts (default: GCP-Secret-Manager)
//! - PACT_CONSUMER: Consumer name in contracts (default: Secret-Manager-Controller)
//! - PORT: Port to listen on (default: 1234)

use axum::{
    extract::{Path, State},
    http::{Method, StatusCode, Uri},
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
// base64 encoding is handled by the secret store
use pact_mock_server::{
    auth_failure_middleware, health_check, load_contracts_from_broker, logging_middleware,
    rate_limit_middleware, service_unavailable_middleware,
    AppState,
};
use pact_mock_server::secrets::common::errors::gcp_error_response;
use pact_mock_server::secrets::common::limits::validate_gcp_secret_size;
use pact_mock_server::secrets::gcp::GcpSecretStore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, warn, Level};

/// Format Unix timestamp (seconds) to RFC3339 format (GCP API format)
fn format_timestamp_rfc3339(timestamp: u64) -> String {
    // Format as RFC3339 (e.g., "2023-01-01T00:00:00Z")
    // Using a simple format since we don't have chrono in dependencies
    // GCP uses format like "2023-01-01T00:00:00.000000Z"
    let secs = timestamp;
    let days = secs / 86400;
    let secs_in_day = secs % 86400;
    let hours = secs_in_day / 3600;
    let minutes = (secs_in_day % 3600) / 60;
    let seconds = secs_in_day % 60;
    
    // Approximate year calculation (simplified, but sufficient for mock)
    let year = 1970 + (days / 365);
    let day_of_year = days % 365;
    let month = 1 + (day_of_year / 30);
    let day = 1 + (day_of_year % 30);
    
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.000000Z", year, month, day, hours, minutes, seconds)
}

/// GCP-specific application state
#[derive(Clone)]
struct GcpAppState {
    #[allow(dead_code)] // Reserved for future contract-based responses
    contracts: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, serde_json::Value>>>,
    secrets: GcpSecretStore,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateSecretRequest {
    #[serde(rename = "secretId")]
    secret_id: String,
    replication: Replication,
}

#[derive(Debug, Serialize, Deserialize)]
struct Replication {
    automatic: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AddVersionRequest {
    payload: SecretPayload,
}

#[derive(Debug, Serialize, Deserialize)]
struct SecretPayload {
    data: String,
}

#[derive(Debug, Serialize)]
struct SecretResponse {
    name: String,
    payload: Option<SecretPayload>,
    replication: Option<Replication>,
    /// Creation timestamp (Unix timestamp in seconds)
    /// GCP includes this in version responses
    #[serde(skip_serializing_if = "Option::is_none")]
    create_time: Option<String>, // RFC3339 format
}

/// GET secret value (access latest version)
/// Path: /v1/projects/{project}/secrets/{secret}/versions/latest:access
async fn get_secret_value_access(
    State(app_state): State<GcpAppState>,
    Path((project, secret)): Path<(String, String)>,
) -> Response {
    info!(
        "  GET secret value (access): project={}, secret={}",
        project, secret
    );

    // Try to retrieve latest version from in-memory store
    if let Some(version) = app_state.secrets.get_latest(&project, &secret).await {
        info!("  Found secret version {} in store: projects/{}/secrets/{}", version.version_id, project, secret);
        
        // Extract the payload from version data
        if let Some(payload_obj) = version.data.get("payload") {
            if let Some(data) = payload_obj.get("data").and_then(|v| v.as_str()) {
                // Convert Unix timestamp to RFC3339 format (GCP API format)
                let create_time = format_timestamp_rfc3339(version.created_at);
                
                let response = SecretResponse {
                    name: format!("projects/{}/secrets/{}/versions/{}", project, secret, version.version_id),
                    payload: Some(SecretPayload {
                        data: data.to_string(),
                    }),
                    replication: None,
                    create_time: Some(create_time),
                };
                return Json(response).into_response();
            }
        }
    }

    // Secret not found in store or no enabled versions, return 404
    warn!("  Secret not found or disabled in store: projects/{}/secrets/{}", project, secret);
    gcp_error_response(
        StatusCode::NOT_FOUND,
        format!("Secret not found: projects/{}/secrets/{}", project, secret),
        Some("NOT_FOUND"),
    )
}

/// Handler for routes with colons in the path (fallback)
/// Handles:
/// - GET /v1/projects/{project}/secrets/{secret}/versions/latest:access
/// - GET /v1/projects/{project}/secrets/{secret}/versions/{version}:access
/// - GET /v1/projects/{project}/secrets/{secret}/versions (list versions)
/// - POST /v1/projects/{project}/secrets/{secret}:addVersion
/// - POST /v1/projects/{project}/secrets/{secret}:disable
/// - POST /v1/projects/{project}/secrets/{secret}:enable
/// - POST /v1/projects/{project}/secrets/{secret}/versions/{version}:disable
/// - POST /v1/projects/{project}/secrets/{secret}/versions/{version}:enable
async fn handle_colon_routes(
    State(app_state): State<GcpAppState>,
    method: Method,
    uri: Uri,
    body: Option<axum::extract::Json<AddVersionRequest>>,
) -> Response {
    let path = uri.path();

    // Handle GET request to path ending with :access
    if method == Method::GET && path.contains(":access") {
        // Parse path: /v1/projects/{project}/secrets/{secret}/versions/latest:access
        // or: /v1/projects/{project}/secrets/{secret}/versions/{version}:access
        let parts: Vec<&str> = path.split('/').collect();
        let project = parts.get(3).unwrap_or(&"unknown").to_string();
        let secret = parts.get(5).unwrap_or(&"unknown").to_string();
        
        // Check if this is a specific version or latest
        if path.contains("/versions/latest:access") {
            return get_secret_value_access(State(app_state.clone()), Path((project, secret))).await;
        } else if path.contains("/versions/") && path.contains(":access") {
            // Specific version: /v1/projects/{project}/secrets/{secret}/versions/{version}:access
            let version_part = parts.get(7).unwrap_or(&"unknown");
            let version_id = version_part.split(':').next().unwrap_or("unknown").to_string();
            
            return get_secret_version_access(State(app_state.clone()), Path((project, secret, version_id))).await;
        }
    }

    // Handle POST request to path ending with :addVersion
    if method == Method::POST && path.contains(":addVersion") {
        // Parse path: /v1/projects/{project}/secrets/{secret}:addVersion
        let parts: Vec<&str> = path.split('/').collect();
        let project = parts.get(3).unwrap_or(&"unknown").to_string();
        let secret_part = parts.get(5).unwrap_or(&"unknown");
        let secret = secret_part.split(':').next().unwrap_or("unknown").to_string();

        if let Some(Json(body)) = body {
            info!("  ADD VERSION: project={}, secret={}", project, secret);
            
            // Validate secret size (GCP limit: 64KB)
            if let Err(size_error) = validate_gcp_secret_size(&body.payload.data) {
                warn!("  Secret size validation failed: {}", size_error);
                return gcp_error_response(
                    StatusCode::BAD_REQUEST,
                    size_error,
                    Some("INVALID_ARGUMENT"),
                );
            }
            
            // Add a new version with the payload data
            let version_data = json!({
                "payload": {
                    "data": body.payload.data
                }
            });
            
            let version_id = app_state.secrets.add_version(
                &project,
                &secret,
                version_data,
                None, // Auto-generate version ID (sequential for GCP)
            ).await;

            // Get the version to include timestamp
            let version = app_state.secrets.get_version(&project, &secret, &version_id).await;
            let create_time = version.as_ref()
                .map(|v| format_timestamp_rfc3339(v.created_at));

            let response = SecretResponse {
                name: format!("projects/{}/secrets/{}/versions/{}", project, secret, version_id),
                payload: Some(body.payload),
                replication: None,
                create_time,
            };

            info!("  Added version {} to mock secret: {}", version_id, secret);
            return Json(response).into_response();
        } else {
            return gcp_error_response(
                StatusCode::BAD_REQUEST,
                "Missing request body".to_string(),
                Some("INVALID_ARGUMENT"),
            );
        }
    }

    // Handle POST request to path ending with :disable (secret or version)
    if method == Method::POST && path.contains(":disable") {
        // Parse path: /v1/projects/{project}/secrets/{secret}:disable
        // or: /v1/projects/{project}/secrets/{secret}/versions/{version}:disable
        let parts: Vec<&str> = path.split('/').collect();
        let project = parts.get(3).unwrap_or(&"unknown").to_string();
        
        if path.contains("/versions/") {
            // Version disable: /v1/projects/{project}/secrets/{secret}/versions/{version}:disable
            let secret = parts.get(5).unwrap_or(&"unknown").to_string();
            let version_part = parts.get(7).unwrap_or(&"unknown");
            let version_id = version_part.split(':').next().unwrap_or("unknown").to_string();
            
            info!("  DISABLE VERSION: project={}, secret={}, version={}", project, secret, version_id);
            
            if app_state.secrets.disable_version(&project, &secret, &version_id).await {
                let response = SecretResponse {
                    name: format!("projects/{}/secrets/{}/versions/{}", project, secret, version_id),
                    payload: None,
                    replication: None,
                    create_time: None,
                };
                return Json(response).into_response();
            } else {
                return gcp_error_response(
                    StatusCode::NOT_FOUND,
                    format!("Version not found: projects/{}/secrets/{}/versions/{}", project, secret, version_id),
                    Some("NOT_FOUND"),
                );
            }
        } else {
            // Secret disable: /v1/projects/{project}/secrets/{secret}:disable
            let secret_part = parts.get(5).unwrap_or(&"unknown");
            let secret = secret_part.split(':').next().unwrap_or("unknown").to_string();
            
            info!("  DISABLE SECRET: project={}, secret={}", project, secret);
            
            if app_state.secrets.disable_secret(&project, &secret).await {
                let response = SecretResponse {
                    name: format!("projects/{}/secrets/{}", project, secret),
                    payload: None,
                    replication: None,
                    create_time: None,
                };
                return Json(response).into_response();
            } else {
                return gcp_error_response(
                    StatusCode::NOT_FOUND,
                    format!("Secret not found: projects/{}/secrets/{}", project, secret),
                    Some("NOT_FOUND"),
                );
            }
        }
    }

    // Handle POST request to path ending with :enable (secret or version)
    if method == Method::POST && path.contains(":enable") {
        // Parse path: /v1/projects/{project}/secrets/{secret}:enable
        // or: /v1/projects/{project}/secrets/{secret}/versions/{version}:enable
        let parts: Vec<&str> = path.split('/').collect();
        let project = parts.get(3).unwrap_or(&"unknown").to_string();
        
        if path.contains("/versions/") {
            // Version enable: /v1/projects/{project}/secrets/{secret}/versions/{version}:enable
            let secret = parts.get(5).unwrap_or(&"unknown").to_string();
            let version_part = parts.get(7).unwrap_or(&"unknown");
            let version_id = version_part.split(':').next().unwrap_or("unknown").to_string();
            
            info!("  ENABLE VERSION: project={}, secret={}, version={}", project, secret, version_id);
            
            if app_state.secrets.enable_version(&project, &secret, &version_id).await {
                let response = SecretResponse {
                    name: format!("projects/{}/secrets/{}/versions/{}", project, secret, version_id),
                    payload: None,
                    replication: None,
                    create_time: None,
                };
                return Json(response).into_response();
            } else {
                return gcp_error_response(
                    StatusCode::NOT_FOUND,
                    format!("Version not found: projects/{}/secrets/{}/versions/{}", project, secret, version_id),
                    Some("NOT_FOUND"),
                );
            }
        } else {
            // Secret enable: /v1/projects/{project}/secrets/{secret}:enable
            let secret_part = parts.get(5).unwrap_or(&"unknown");
            let secret = secret_part.split(':').next().unwrap_or("unknown").to_string();
            
            info!("  ENABLE SECRET: project={}, secret={}", project, secret);
            
            if app_state.secrets.enable_secret(&project, &secret).await {
                let response = SecretResponse {
                    name: format!("projects/{}/secrets/{}", project, secret),
                    payload: None,
                    replication: None,
                    create_time: None,
                };
                return Json(response).into_response();
            } else {
                return gcp_error_response(
                    StatusCode::NOT_FOUND,
                    format!("Secret not found: projects/{}/secrets/{}", project, secret),
                    Some("NOT_FOUND"),
                );
            }
        }
    }

    // Handle GET request to list versions
    if method == Method::GET && path.ends_with("/versions") && !path.contains(":") {
        // Parse path: /v1/projects/{project}/secrets/{secret}/versions
        let parts: Vec<&str> = path.split('/').collect();
        let project = parts.get(3).unwrap_or(&"unknown").to_string();
        let secret = parts.get(5).unwrap_or(&"unknown").to_string();
        
        return list_secret_versions(State(app_state.clone()), Path((project, secret))).await;
    }

    // Not a colon route, return 404
    warn!("  ⚠️  Unmatched route: {} {}", method, path);
    gcp_error_response(
        StatusCode::NOT_FOUND,
        format!("Route not found: {} {}", method, path),
        Some("NOT_FOUND"),
    )
}

/// GET secret value (access specific version)
/// Path: /v1/projects/{project}/secrets/{secret}/versions/{version}:access
async fn get_secret_version_access(
    State(app_state): State<GcpAppState>,
    Path((project, secret, version_id)): Path<(String, String, String)>,
) -> Response {
    info!(
        "  GET secret version (access): project={}, secret={}, version={}",
        project, secret, version_id
    );

    // Try to retrieve specific version from in-memory store
    if let Some(version) = app_state.secrets.get_version(&project, &secret, &version_id).await {
        // Check if version is enabled
        if !version.enabled {
            warn!("  Version {} is disabled: projects/{}/secrets/{}/versions/{}", version_id, project, secret, version_id);
            return gcp_error_response(
                StatusCode::NOT_FOUND,
                format!("Version not found or disabled: projects/{}/secrets/{}/versions/{}", project, secret, version_id),
                Some("NOT_FOUND"),
            );
        }
        
        info!("  Found secret version {} in store: projects/{}/secrets/{}/versions/{}", version_id, project, secret, version_id);
        
        // Extract the payload from version data
        if let Some(payload_obj) = version.data.get("payload") {
            if let Some(data) = payload_obj.get("data").and_then(|v| v.as_str()) {
                // Convert Unix timestamp to RFC3339 format (GCP API format)
                let create_time = format_timestamp_rfc3339(version.created_at);
                
                let response = SecretResponse {
                    name: format!("projects/{}/secrets/{}/versions/{}", project, secret, version_id),
                    payload: Some(SecretPayload {
                        data: data.to_string(),
                    }),
                    replication: None,
                    create_time: Some(create_time),
                };
                return Json(response).into_response();
            }
        }
    }

    // Version not found, return 404
    warn!("  Version not found in store: projects/{}/secrets/{}/versions/{}", project, secret, version_id);
    gcp_error_response(
        StatusCode::NOT_FOUND,
        format!("Version not found: projects/{}/secrets/{}/versions/{}", project, secret, version_id),
        Some("NOT_FOUND"),
    )
}

/// GET list of secret versions
/// Path: /v1/projects/{project}/secrets/{secret}/versions
async fn list_secret_versions(
    State(app_state): State<GcpAppState>,
    Path((project, secret)): Path<(String, String)>,
) -> Response {
    info!(
        "  GET secret versions list: project={}, secret={}",
        project, secret
    );

    // Check if secret exists
    if !app_state.secrets.exists(&project, &secret).await {
        warn!("  Secret not found: projects/{}/secrets/{}", project, secret);
        return gcp_error_response(
            StatusCode::NOT_FOUND,
            format!("Secret not found: projects/{}/secrets/{}", project, secret),
            Some("NOT_FOUND"),
        );
    }

    // Get all versions
    if let Some(versions) = app_state.secrets.list_versions(&project, &secret).await {
        let version_list: Vec<serde_json::Value> = versions
            .iter()
            .map(|v| {
                json!({
                    "name": format!("projects/{}/secrets/{}/versions/{}", project, secret, v.version_id),
                    "createTime": format_timestamp_rfc3339(v.created_at),
                    "state": if v.enabled { "ENABLED" } else { "DISABLED" }
                })
            })
            .collect();

        Json(json!({
            "versions": version_list
        }))
        .into_response()
    } else {
        // No versions found, return empty list
        Json(json!({
            "versions": []
        }))
        .into_response()
    }
}

/// CREATE secret
async fn create_secret(
    State(app_state): State<GcpAppState>,
    Path(project): Path<String>,
    Json(body): Json<CreateSecretRequest>,
) -> Json<SecretResponse> {
    info!("  CREATE secret: project={}, secret_id={}", project, body.secret_id);
    
    // Store the secret metadata (replication config)
    // The secret will be created when the first version is added
    let metadata = json!({
        "replication": body.replication
    });
    app_state.secrets.update_metadata(&project, &body.secret_id, metadata).await;

    let response = SecretResponse {
        name: format!("projects/{}/secrets/{}", project, body.secret_id),
        payload: None,
        replication: Some(body.replication),
        create_time: None, // Secret metadata doesn't include version timestamps
    };

    info!("  Created mock secret and stored: {}", body.secret_id);
    Json(response)
}

/// GET secret metadata
/// Path: /v1/projects/{project}/secrets/{secret}
async fn get_secret_metadata(
    State(app_state): State<GcpAppState>,
    Path((project, secret)): Path<(String, String)>,
) -> Response {
    info!("  GET secret metadata: project={}, secret={}", project, secret);

    // Try to retrieve metadata from in-memory store
    if let Some(metadata) = app_state.secrets.get_metadata(&project, &secret).await {
        info!("  Found secret metadata in store: projects/{}/secrets/{}", project, secret);
        
        // Extract replication from metadata
        let replication = metadata
            .get("replication")
            .and_then(|r| serde_json::from_value(r.clone()).ok())
            .unwrap_or_else(|| Replication {
                automatic: Some(json!({})),
            });

        let response = SecretResponse {
            name: format!("projects/{}/secrets/{}", project, secret),
            payload: None,
            replication: Some(replication),
            create_time: None, // Secret metadata doesn't include version timestamps
        };

        return Json(response).into_response();
    }

    // Secret not found in store, return 404
    warn!("  Secret not found in store: projects/{}/secrets/{}", project, secret);
    gcp_error_response(
        StatusCode::NOT_FOUND,
        format!("Secret not found: projects/{}/secrets/{}", project, secret),
        Some("NOT_FOUND"),
    )
}

/// DELETE secret
/// Path: /v1/projects/{project}/secrets/{secret}
async fn delete_secret(
    State(app_state): State<GcpAppState>,
    Path((project, secret)): Path<(String, String)>,
) -> StatusCode {
    info!("  DELETE secret: project={}, secret={}", project, secret);
    
    if app_state.secrets.delete_secret(&project, &secret).await {
        info!("  Deleted secret from store: {}", secret);
        StatusCode::OK
    } else {
        warn!("  Secret not found in store: projects/{}/secrets/{}", project, secret);
        StatusCode::NOT_FOUND
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    // Load configuration from environment
    let broker_url = env::var("PACT_BROKER_URL")
        .unwrap_or_else(|_| "http://pact-broker:9292".to_string());
    let username = env::var("PACT_BROKER_USERNAME").unwrap_or_else(|_| "pact".to_string());
    let password = env::var("PACT_BROKER_PASSWORD").unwrap_or_else(|_| "pact".to_string());
    let provider = env::var("PACT_PROVIDER")
        .unwrap_or_else(|_| "GCP-Secret-Manager".to_string());
    let consumer = env::var("PACT_CONSUMER")
        .unwrap_or_else(|_| "Secret-Manager-Controller".to_string());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "1234".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid u16");

    info!("Starting GCP Secret Manager Mock Server...");
    info!("Broker URL: {}", broker_url);
    info!("Provider: {}, Consumer: {}", provider, consumer);

    // Load contracts from broker
    let contracts =
        load_contracts_from_broker(&broker_url, &username, &password, &provider, &consumer).await;
    if contracts.is_empty() {
        warn!("⚠️  No contracts loaded, using default mock responses");
    }

    let contracts_state = AppState::new(contracts);
    let app_state = GcpAppState {
        contracts: contracts_state.contracts,
        secrets: GcpSecretStore::new(),
    };

    // Build router with explicit routes for all GCP Secret Manager API endpoints
    let app = Router::new()
        // Health check endpoints
        .route("/", get(health_check))
        .route("/health", get(health_check))
        // GCP Secret Manager API endpoints
        // POST /v1/projects/{project}/secrets - Create a new secret
        .route("/v1/projects/{project}/secrets", post(create_secret))
        // GET /v1/projects/{project}/secrets/{secret}/versions/latest:access - Get secret value (access latest)
        // Note: The colon in the path requires using fallback handler
        // This route is handled by the fallback handler which parses the path manually
        // DELETE /v1/projects/{project}/secrets/{secret} - Delete secret
        .route(
            "/v1/projects/{project}/secrets/{secret}",
            delete(delete_secret).get(get_secret_metadata),
        )
        // POST /v1/projects/{project}/secrets/{secret}:addVersion - Add a new version
        .fallback(handle_colon_routes)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(axum::middleware::from_fn(auth_failure_middleware))
                .layer(axum::middleware::from_fn(service_unavailable_middleware))
                .layer(axum::middleware::from_fn(rate_limit_middleware))
                .layer(axum::middleware::from_fn(logging_middleware)),
        )
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on port {}", port);
    info!("✅ GCP Mock server ready at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

