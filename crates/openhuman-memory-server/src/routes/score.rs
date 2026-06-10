//! `memory.score_chunk` — score a chunk of text using the memory tree scorer.

use serde_json::Value;

use openhuman_memory::tree::score::{score_chunk, ScoringConfig};
use openhuman_memory::store::chunks::types::{Chunk, Metadata, SourceKind};

use crate::AppState;
use crate::error::RpcError;

pub async fn handle_score_chunk(_state: &AppState, params: Value) -> Result<Value, RpcError> {
    let text = params
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_params("missing field: text"))?
        .to_string();

    let source_kind_str = params
        .get("source_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("document");

    let source_kind =
        SourceKind::parse(source_kind_str).map_err(RpcError::invalid_params)?;

    let now = chrono::Utc::now();
    let token_count = (text.len() / 4).max(1) as u32;

    let metadata = Metadata::point_in_time(source_kind, "rpc_score", "", now);

    let chunk = Chunk {
        id: "rpc_score_chunk".to_string(),
        content: text,
        token_count,
        seq_in_source: 0,
        created_at: now,
        metadata,
        partial_message: false,
    };

    let cfg = ScoringConfig::default_regex_only();
    let result = score_chunk(&chunk, &cfg)
        .await
        .map_err(|e| RpcError::internal(format!("score_chunk failed: {e}")))?;

    tracing::debug!(
        "[rpc] score_chunk source_kind={source_kind_str} tokens={token_count} kept={} total={:.3}",
        result.kept,
        result.total
    );

    serde_json::to_value(&result).map_err(|e| RpcError::internal(format!("serialize: {e}")))
}
