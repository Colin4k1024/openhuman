//! Agent abstraction — hooks, triage, session transcripts, prompts.
//!
//! Replaces `crate::openhuman::agent::hooks`, `agent::triage`,
//! `agent::harness::session::transcript`, and `agent::prompts` usage.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Minimal representation of a tool call within a turn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub name: String,
    pub tool_name: String,
    pub input: Option<serde_json::Value>,
    pub output: Option<String>,
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
    /// Called when a turn completes.
    async fn on_turn_complete(&self, ctx: &TurnContext) -> anyhow::Result<()>;
    /// Legacy alias (default delegates to on_turn_complete).
    async fn after_turn(&self, ctx: &TurnContext) -> anyhow::Result<()> {
        self.on_turn_complete(ctx).await
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

/// Prompt context section trait for injection into agent prompts.
pub trait PromptSection: Send + Sync {
    fn name(&self) -> &str;
    fn build(&self, ctx: &PromptContext<'_>) -> anyhow::Result<String>;
    fn priority(&self) -> i32 { 0 }
}

/// Prompt context with lifetime (passed to PromptSection::build).
#[derive(Clone, Debug, Default)]
pub struct PromptContext<'a> {
    pub config: Option<&'a crate::config::Config>,
    pub thread_id: Option<&'a str>,
    pub learned: Option<&'a LearnedContextData>,
}

/// Context data for learned knowledge injection.
#[derive(Clone, Debug, Default)]
pub struct LearnedContextData {
    pub sections: Vec<String>,
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
    fn sections(&self) -> Vec<Box<dyn PromptSection>>;
}

/// Prompt module stubs.
pub mod prompt {
    pub use super::{LearnedContextData, PromptContext, PromptSection, ToolCallFormat};
}

/// Triage module stubs.
pub mod triage {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    pub struct TriageOutcome {
        pub action: String,
    }

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    pub struct TriggerEnvelope {
        pub source: String,
        pub payload: serde_json::Value,
    }

    pub async fn run_triage(_config: &crate::config::Config) -> anyhow::Result<()> {
        Ok(())
    }

    pub async fn apply_decision(_config: &crate::config::Config, _decision: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Agent harness stubs.
pub mod harness {
    pub mod session {
        pub mod transcript {
            pub use crate::bridge::agent::{SessionTranscript, TranscriptMessage};

            /// Read a transcript from a file path (stub).
            pub fn read_transcript(
                _path: &std::path::Path,
            ) -> anyhow::Result<crate::bridge::agent::SessionTranscript> {
                Ok(crate::bridge::agent::SessionTranscript::default())
            }
        }
    }
}
