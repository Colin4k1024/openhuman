//! Memory queue stubs — will be fully implemented when memory_queue is moved in.

use rusqlite::Transaction;

pub mod types {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct ReembedBackfillPayload {
        pub signature: String,
    }

    #[derive(Clone, Debug)]
    pub struct NewJob {
        pub kind: String,
        pub payload: String,
    }

    impl NewJob {
        pub fn reembed_backfill(payload: &ReembedBackfillPayload) -> anyhow::Result<Self> {
            Ok(Self {
                kind: "reembed_backfill".into(),
                payload: serde_json::to_string(payload).unwrap_or_default(),
            })
        }
    }
}

/// Enqueue a job within a transaction (stub).
pub fn enqueue_tx(_tx: &Transaction<'_>, _job: &types::NewJob) -> anyhow::Result<()> {
    Ok(())
}

/// Mark backfill as in progress (stub).
pub fn set_backfill_in_progress(_in_progress: bool) {}
