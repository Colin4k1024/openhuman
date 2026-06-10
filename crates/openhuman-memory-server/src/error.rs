//! JSON-RPC error type and axum `IntoResponse` impl.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl RpcError {
    /// -32700 Parse error
    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self { code: -32700, message: msg.into() }
    }

    /// -32600 Invalid Request
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self { code: -32600, message: msg.into() }
    }

    /// -32601 Method not found
    pub fn method_not_found(method: &str) -> Self {
        Self { code: -32601, message: format!("method not found: {method}") }
    }

    /// -32602 Invalid params
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self { code: -32602, message: msg.into() }
    }

    /// -32603 Internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self { code: -32603, message: msg.into() }
    }
}

/// Wraps an `RpcError` together with the JSON-RPC request id so it can be
/// returned from axum handlers via `IntoResponse`.
pub struct RpcErrorResponse {
    pub error: RpcError,
    pub id: Value,
}

impl IntoResponse for RpcErrorResponse {
    fn into_response(self) -> Response {
        let body = json!({
            "jsonrpc": "2.0",
            "error": {
                "code": self.error.code,
                "message": self.error.message
            },
            "id": self.id
        });
        // Always 200 for well-formed JSON-RPC error objects; use 500 for
        // transport-level failures (parse errors) where we have no id.
        let status = if self.id.is_null() && self.error.code == -32700 {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::OK
        };
        (status, Json(body)).into_response()
    }
}
