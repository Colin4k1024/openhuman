//! `openhuman-memory-server` — standalone HTTP / JSON-RPC server for the
//! openhuman-memory system.
//!
//! ## Usage
//!
//! ```sh
//! openhuman-memory-server \
//!   --port 8420 \
//!   --token mysecret \
//!   --workspace ~/.openhuman-memory-server
//! ```
//!
//! Or set `MEMORY_SERVER_TOKEN` in the environment and omit `--token`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use clap::Parser;
use openhuman_memory::{
    embedding_ext::default_embedding_provider, store::UnifiedMemory,
};
use tracing::info;

use openhuman_memory_server::{build_app, AppState};

#[derive(Parser, Debug)]
#[command(name = "openhuman-memory-server", about = "Standalone memory HTTP/JSON-RPC server")]
struct Args {
    /// TCP port to listen on.
    #[arg(long, default_value_t = 8420)]
    port: u16,

    /// Bearer token for API authentication. Falls back to env `MEMORY_SERVER_TOKEN`.
    #[arg(long, env = "MEMORY_SERVER_TOKEN")]
    token: String,

    /// Workspace directory for the memory store.
    #[arg(long, default_value = "~/.openhuman-memory-server")]
    workspace: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise structured tracing from `RUST_LOG` or a sensible default.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "openhuman_memory_server=debug,info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    // Expand `~` in workspace path.
    let workspace = expand_tilde(&args.workspace);
    std::fs::create_dir_all(&workspace)
        .with_context(|| format!("create workspace dir: {}", workspace.display()))?;

    info!("[server] workspace={} port={}", workspace.display(), args.port);

    // Create the embedding provider (Ollama by default; honours env overrides).
    let embedder = default_embedding_provider();

    // Open / create the unified memory store.
    let memory = UnifiedMemory::new(&workspace, embedder, None)
        .with_context(|| format!("open UnifiedMemory at {}", workspace.display()))?;

    let config = openhuman_memory::config::Config {
        workspace_dir: workspace.clone(),
        ..Default::default()
    };

    let state = Arc::new(AppState {
        memory,
        config,
        workspace_dir: workspace.clone(),
    });

    let app = build_app(state, args.token.clone());

    let addr = format!("0.0.0.0:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    info!("[server] listening on {addr}");

    axum::serve(listener, app).await.context("axum serve")?;
    Ok(())
}

/// Expand a leading `~` to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs_next_home() {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

fn dirs_next_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}
