//! Bearer token authentication middleware.

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};

/// Axum middleware that enforces `Authorization: Bearer <token>` on every request.
/// The expected token is stored in a request extension set by `AppState`.
pub async fn bearer_auth(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = request
        .extensions()
        .get::<ExpectedToken>()
        .map(|t| t.0.as_str())
        .unwrap_or("");

    let provided = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if provided.is_empty() || provided != expected {
        tracing::warn!("[auth] rejected request: missing or invalid bearer token");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

/// Extension type carrying the expected bearer token into the middleware.
#[derive(Clone)]
pub struct ExpectedToken(pub String);
