//! Python bindings for openhuman-memory.
//!
//! Exposes MemoryStore (documents, KV, namespaces) and scoring via PyO3.
//! Uses a blocking tokio runtime internally since PyO3 async is complex.

use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use std::path::PathBuf;
use std::sync::Arc;

use openhuman_memory::config::Config;
use openhuman_memory::embedding_ext::default_embedding_provider;
use openhuman_memory::store::UnifiedMemory;
use openhuman_memory::store::types::NamespaceDocumentInput;

/// Configuration for the memory system.
#[pyclass(name = "Config")]
#[derive(Clone)]
struct PyConfig {
    inner: Config,
}

#[pymethods]
impl PyConfig {
    #[new]
    #[pyo3(signature = (workspace=None))]
    fn new(workspace: Option<&str>) -> PyResult<Self> {
        let workspace_dir = match workspace {
            Some(p) => PathBuf::from(p),
            None => dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".openhuman-memory"),
        };
        std::fs::create_dir_all(&workspace_dir)
            .map_err(|e| PyRuntimeError::new_err(format!("cannot create workspace: {e}")))?;
        Ok(Self {
            inner: Config {
                workspace_dir,
                ..Config::default()
            },
        })
    }

    /// Return the workspace directory path.
    #[getter]
    fn workspace(&self) -> String {
        self.inner.workspace_dir.display().to_string()
    }
}

/// Main memory store — wraps UnifiedMemory with a tokio runtime.
#[pyclass(name = "MemoryStore")]
struct PyMemoryStore {
    memory: Arc<UnifiedMemory>,
    runtime: tokio::runtime::Runtime,
}

#[pymethods]
impl PyMemoryStore {
    #[new]
    fn new(config: &PyConfig) -> PyResult<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("tokio runtime: {e}")))?;
        let embedder = default_embedding_provider();
        let memory = UnifiedMemory::new(&config.inner.workspace_dir, embedder, None)
            .map_err(|e| PyRuntimeError::new_err(format!("memory init: {e}")))?;
        Ok(Self {
            memory: Arc::new(memory),
            runtime,
        })
    }

    /// Insert or update a document. Returns the document ID.
    fn upsert_document(
        &self,
        namespace: &str,
        key: &str,
        title: &str,
        content: &str,
    ) -> PyResult<String> {
        let input = NamespaceDocumentInput {
            namespace: namespace.into(),
            key: key.into(),
            title: title.into(),
            content: content.into(),
            source_type: "python".into(),
            priority: "normal".into(),
            tags: vec![],
            metadata: serde_json::json!({}),
            category: "knowledge".into(),
            session_id: None,
            document_id: None,
            taint: Default::default(),
        };
        let mem = self.memory.clone();
        self.runtime
            .block_on(async move { mem.upsert_document(input).await })
            .map_err(|e| PyRuntimeError::new_err(e))
    }

    /// List documents in a namespace. Returns JSON string.
    #[pyo3(signature = (namespace=None))]
    fn list_documents(&self, namespace: Option<&str>) -> PyResult<String> {
        let mem = self.memory.clone();
        let ns = namespace.map(|s| s.to_string());
        let result = self
            .runtime
            .block_on(async move { mem.list_documents(ns.as_deref()).await })
            .map_err(|e| PyRuntimeError::new_err(e))?;
        Ok(result.to_string())
    }

    /// List all namespaces.
    fn list_namespaces(&self) -> PyResult<Vec<String>> {
        let mem = self.memory.clone();
        self.runtime
            .block_on(async move { mem.list_namespaces().await })
            .map_err(|e| PyRuntimeError::new_err(e))
    }

    /// Clear all documents in a namespace.
    fn clear_namespace(&self, namespace: &str) -> PyResult<()> {
        let mem = self.memory.clone();
        let ns = namespace.to_string();
        self.runtime
            .block_on(async move { mem.clear_namespace(&ns).await })
            .map_err(|e| PyRuntimeError::new_err(e))
    }

    /// Set a global key-value pair. Value is a JSON string.
    fn kv_set(&self, key: &str, value_json: &str) -> PyResult<()> {
        let val: serde_json::Value = serde_json::from_str(value_json)
            .map_err(|e| PyRuntimeError::new_err(format!("invalid JSON: {e}")))?;
        let mem = self.memory.clone();
        let k = key.to_string();
        self.runtime
            .block_on(async move { mem.kv_set_global(&k, &val).await })
            .map_err(|e| PyRuntimeError::new_err(e))
    }

    /// Get a global key-value pair. Returns JSON string or None.
    fn kv_get(&self, key: &str) -> PyResult<Option<String>> {
        let mem = self.memory.clone();
        let k = key.to_string();
        let result = self
            .runtime
            .block_on(async move { mem.kv_get_global(&k).await })
            .map_err(|e| PyRuntimeError::new_err(e))?;
        Ok(result.map(|v| v.to_string()))
    }

    /// Delete a global key-value pair.
    fn kv_delete(&self, key: &str) -> PyResult<()> {
        let mem = self.memory.clone();
        let k = key.to_string();
        self.runtime
            .block_on(async move { mem.kv_delete_global(&k).await })
            .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;
        Ok(())
    }
}

/// Score a text chunk using the regex-only pipeline. Returns JSON of ScoreResult.
#[pyfunction]
#[pyo3(signature = (text, source_kind=None))]
fn score_chunk(text: &str, source_kind: Option<&str>) -> PyResult<String> {
    use openhuman_memory::store::chunks::types::{Chunk, Metadata, SourceKind};
    use openhuman_memory::tree::score::{score_chunk as do_score, ScoringConfig};

    let kind = match source_kind.unwrap_or("chat") {
        "email" => SourceKind::Email,
        "document" => SourceKind::Document,
        _ => SourceKind::Chat,
    };

    let now = chrono::Utc::now();
    let chunk = Chunk {
        id: format!("py-{}", uuid_simple()),
        content: text.to_string(),
        metadata: Metadata {
            source_kind: kind,
            source_id: "python-api".into(),
            owner: String::new(),
            timestamp: now,
            time_range: (now, now),
            tags: vec![],
            source_ref: None,
            path_scope: None,
        },
        token_count: (text.len() / 4) as u32,
        seq_in_source: 0,
        created_at: now,
        partial_message: false,
    };

    let cfg = ScoringConfig::default_regex_only();
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| PyRuntimeError::new_err(format!("runtime: {e}")))?;
    let result = rt
        .block_on(do_score(&chunk, &cfg))
        .map_err(|e| PyRuntimeError::new_err(format!("score error: {e}")))?;

    let json = serde_json::json!({
        "chunk_id": result.chunk_id,
        "total": result.total,
        "kept": result.kept,
        "drop_reason": result.drop_reason,
        "entities": result.extracted.entities.iter()
            .map(|e| serde_json::json!({"text": e.text, "kind": format!("{:?}", e.kind)}))
            .collect::<Vec<_>>(),
    });

    Ok(json.to_string())
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{:x}{:x}", t.as_secs(), t.subsec_nanos())
}

/// Python module definition.
#[pymodule(name = "openhuman_memory")]
fn openhuman_memory_python(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConfig>()?;
    m.add_class::<PyMemoryStore>()?;
    m.add_function(wrap_pyfunction!(score_chunk, m)?)?;
    Ok(())
}
