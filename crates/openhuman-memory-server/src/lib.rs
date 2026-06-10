//! Library entry-point for `openhuman-memory-server`.
//!
//! Exposes [`AppState`] and [`build_app`] so that the `tests/` integration
//! suite can construct a real in-process server without going through the CLI
//! argument parser.

mod auth;
mod error;
mod routes;

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::Request,
    middleware::{self, Next},
    routing::post,
    Router,
};
use openhuman_memory::store::UnifiedMemory;
use tower_http::trace::TraceLayer;

use auth::{bearer_auth, ExpectedToken};

/// Shared application state injected into every handler.
pub struct AppState {
    /// The unified memory store (documents + KV + graph + vectors).
    pub memory: UnifiedMemory,
    /// Workspace root directory (used to build entity paths).
    pub workspace_dir: PathBuf,
}

/// Build the axum [`Router`] for a given state + bearer token.
///
/// Extracted so integration tests can reuse the exact same router without
/// going through CLI argument parsing.
pub fn build_app(state: Arc<AppState>, token: String) -> Router {
    Router::new()
        .route("/rpc", post(routes::rpc_handler))
        .layer(middleware::from_fn(move |mut req: Request, next: Next| {
            let t = token.clone();
            async move {
                req.extensions_mut().insert(ExpectedToken(t));
                bearer_auth(req.headers().clone(), req, next).await
            }
        }))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
