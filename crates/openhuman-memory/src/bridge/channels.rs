//! Channel abstraction for conversation memory.
//!
//! Replaces `crate::openhuman::channels::context` and `channels::traits`.

use serde::{Deserialize, Serialize};

/// A message from a channel (Slack, Discord, etc.).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub id: String,
    /// Display name / identifier of the sender.
    pub sender: String,
    /// Who this message is directed at (reply target).
    pub reply_target: String,
    pub content: String,
    /// Unix timestamp in seconds.
    pub timestamp: i64,
    /// Channel identifier (name or ID).
    pub channel: String,
    /// Slack-style thread timestamp for threaded replies.
    pub thread_ts: Option<String>,
    // Legacy optional fields for backwards compat.
    pub sender_name: Option<String>,
    pub sender_id: Option<String>,
    pub timestamp_ms: Option<i64>,
    pub channel_id: Option<String>,
}

/// Build the conversation-history storage key for a channel message.
pub fn conversation_history_key(msg: &ChannelMessage) -> String {
    format!(
        "conversation_history:{}:{}:{}",
        msg.channel, msg.sender, msg.reply_target
    )
}

/// Context module stub.
pub mod context {
    pub use super::conversation_history_key;
}

/// Traits module stub.
pub mod traits {
    pub use super::ChannelMessage;
}
