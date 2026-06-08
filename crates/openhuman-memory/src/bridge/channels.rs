//! Channel abstraction for conversation memory.
//!
//! Replaces `crate::openhuman::channels::context` and `channels::traits`.

use serde::{Deserialize, Serialize};

/// A message from a channel (Slack, Discord, etc.).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub id: String,
    pub sender_name: Option<String>,
    pub sender_id: Option<String>,
    pub content: String,
    pub timestamp_ms: i64,
    pub channel_id: Option<String>,
}

/// Build the conversation-history storage key for a channel.
pub fn conversation_history_key(channel_type: &str, channel_id: &str) -> String {
    format!("conversation_history:{}:{}", channel_type, channel_id)
}

/// Context module stub.
pub mod context {
    pub use super::conversation_history_key;
}

/// Traits module stub.
pub mod traits {
    pub use super::ChannelMessage;
}
