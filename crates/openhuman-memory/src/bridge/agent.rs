//! Agent abstraction — hooks, triage, session transcripts, prompts.
//!
//! Replaces `crate::openhuman::agent::hooks`, `agent::triage`,
//! `agent::harness::session::transcript`, and `agent::prompts` usage.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Minimal representation of a tool call within a turn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool_name: String,
    pub input_summary: Option<String>,
    pub output_summary: Option<String>,
}

/// Context passed to post-turn hooks.
#[derive(Clone, Debug)]
pub struct TurnContext {
    pub thread_id: String,
    pub user_message: Option<String>,
    pub assistant_response: Option<String>,
    pub tool_calls: Vec<ToolCallRecord>,
}

/// Post-turn hook trait (learning hooks implement this).
#[async_trait]
pub trait PostTurnHook: Send + Sync {
    fn name(&self) -> &str;
    async fn after_turn(&self, ctx: &TurnContext) -> anyhow::Result<()>;
    /// Called when a turn completes (optional).
    async fn on_turn_complete(&self, _ctx: &TurnContext) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Session transcript — a parsed conversation record.
#[derive(Clone, Debug, Default)]
pub struct SessionTranscript {
    pub session_id: String,
    pub messages: Vec<TranscriptMessage>,
}

/// A single message in a session transcript.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscriptMessage {
    pub role: String,
    pub content: String,
    pub timestamp_ms: Option<i64>,
}

/// Prompt context section for injection into agent prompts.
#[derive(Clone, Debug)]
pub struct PromptSection {
    pub label: String,
    pub content: String,
    pub priority: i32,
}

/// Context data for learned knowledge injection.
#[derive(Clone, Debug, Default)]
pub struct LearnedContextData {
    pub sections: Vec<PromptSection>,
}

/// Tool call format enum for prompt building.
#[derive(Clone, Debug, Default)]
pub enum ToolCallFormat {
    #[default]
    PFormat,
    JsonFormat,
}

/// Trait for components that provide prompt context.
pub trait PromptContextProvider: Send + Sync {
    fn sections(&self) -> Vec<PromptSection>;
}
