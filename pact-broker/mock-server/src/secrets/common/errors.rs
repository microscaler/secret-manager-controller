//! Provider-specific error response formatting
//!
//! Each cloud provider has a different error response format:
//! - GCP: Uses `{"error": {"code": 404, "message": "...", "status": "NOT_FOUND"}}`
//! - AWS: Uses `{"__type": "ResourceNotFoundException", "message": "..."}`
//! - Azure: Uses `{"error": {"code": "BadParameter", "message": "..."}}`

use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde_json::json;

/// GCP error response format
/// 
/// Format: `{"error": {"code": 404, "message": "...", "status": "NOT_FOUND"}}`
/// Reference: https://cloud.google.com/apis/design/errors
pub fn gcp_error_response(status: StatusCode, message: String, status_string: Option<&str>) -> Response {
    let status_str = status_string.unwrap_or_else(|| {
        match status {
            StatusCode::NOT_FOUND => "NOT_FOUND",
            StatusCode::UNAUTHORIZED => "UNAUTHENTICATED",
            StatusCode::FORBIDDEN => "PERMISSION_DENIED",
            StatusCode::BAD_REQUEST => "INVALID_ARGUMENT",
            StatusCode::TOO_MANY_REQUESTS => "RESOURCE_EXHAUSTED",
            StatusCode::SERVICE_UNAVAILABLE => "UNAVAILABLE",
            StatusCode::INTERNAL_SERVER_ERROR => "INTERNAL",
            _ => "UNKNOWN",
        }
    });

    (
        status,
        Json(json!({
            "error": {
                "code": status.as_u16(),
                "message": message,
                "status": status_str
            }
        })),
    )
        .into_response()
}

/// AWS error response format
/// 
/// Format: `{"__type": "ResourceNotFoundException", "message": "..."}`
/// Reference: https://docs.aws.amazon.com/apigateway/latest/developerguide/handle-errors-in-lambda.html
pub fn aws_error_response(status: StatusCode, error_type: &str, message: String) -> Response {
    (
        status,
        Json(json!({
            "__type": error_type,
            "message": message
        })),
    )
        .into_response()
}

/// AWS error type constants
pub mod aws_error_types {
    pub const RESOURCE_NOT_FOUND: &str = "ResourceNotFoundException";
    pub const INVALID_PARAMETER: &str = "InvalidParameterException";
    pub const INVALID_REQUEST: &str = "InvalidRequestException";
    pub const LIMIT_EXCEEDED: &str = "LimitExceededException";
    pub const INTERNAL_SERVICE: &str = "InternalServiceError";
    pub const INVALID_NEXT_TOKEN: &str = "InvalidNextTokenException";
    pub const DECRYPTION_FAILURE: &str = "DecryptionFailureException";
}

/// Map HTTP status code to AWS error type
pub fn aws_error_type_from_status(status: StatusCode) -> &'static str {
    match status {
        StatusCode::NOT_FOUND => aws_error_types::RESOURCE_NOT_FOUND,
        StatusCode::BAD_REQUEST => aws_error_types::INVALID_PARAMETER,
        StatusCode::UNAUTHORIZED => aws_error_types::INVALID_REQUEST,
        StatusCode::FORBIDDEN => aws_error_types::INVALID_REQUEST,
        StatusCode::TOO_MANY_REQUESTS => aws_error_types::LIMIT_EXCEEDED,
        StatusCode::SERVICE_UNAVAILABLE => aws_error_types::INTERNAL_SERVICE,
        StatusCode::INTERNAL_SERVER_ERROR => aws_error_types::INTERNAL_SERVICE,
        _ => aws_error_types::INVALID_REQUEST,
    }
}

/// Azure error response format
/// 
/// Format: `{"error": {"code": "BadParameter", "message": "..."}}`
/// Reference: https://learn.microsoft.com/en-us/rest/api/azure/
pub fn azure_error_response(status: StatusCode, error_code: &str, message: String) -> Response {
    (
        status,
        Json(json!({
            "error": {
                "code": error_code,
                "message": message
            }
        })),
    )
        .into_response()
}

/// Azure error code constants
pub mod azure_error_codes {
    pub const SECRET_NOT_FOUND: &str = "SecretNotFound";
    pub const BAD_PARAMETER: &str = "BadParameter";
    pub const UNAUTHORIZED: &str = "Unauthorized";
    pub const FORBIDDEN: &str = "Forbidden";
    pub const THROTTLED: &str = "ThrottledRequests";
    pub const SERVICE_UNAVAILABLE: &str = "ServiceUnavailable";
    pub const INTERNAL_ERROR: &str = "InternalError";
}

/// Map HTTP status code to Azure error code
pub fn azure_error_code_from_status(status: StatusCode) -> &'static str {
    match status {
        StatusCode::NOT_FOUND => azure_error_codes::SECRET_NOT_FOUND,
        StatusCode::BAD_REQUEST => azure_error_codes::BAD_PARAMETER,
        StatusCode::UNAUTHORIZED => azure_error_codes::UNAUTHORIZED,
        StatusCode::FORBIDDEN => azure_error_codes::FORBIDDEN,
        StatusCode::TOO_MANY_REQUESTS => azure_error_codes::THROTTLED,
        StatusCode::SERVICE_UNAVAILABLE => azure_error_codes::SERVICE_UNAVAILABLE,
        StatusCode::INTERNAL_SERVER_ERROR => azure_error_codes::INTERNAL_ERROR,
        _ => azure_error_codes::BAD_PARAMETER,
    }
}

