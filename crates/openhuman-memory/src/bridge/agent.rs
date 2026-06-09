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
    /// The arguments passed to the tool.
    #[serde(default)]
    pub arguments: serde_json::Value,
    /// Whether the tool execution reported success.
    #[serde(default)]
    pub success: bool,
    /// Duration of the specific tool execution (ms).
    #[serde(default)]
    pub duration_ms: u64,
}

/// Context passed to post-turn hooks.
#[derive(Clone, Debug)]
pub struct TurnContext {
    pub thread_id: String,
    pub user_message: String,
    pub assistant_response: String,
    pub tool_calls: Vec<ToolCallRecord>,
    /// Total wall-clock time the turn took to resolve (ms).
    pub turn_duration_ms: u64,
    /// Optional session identifier for tracking across multiple turns.
    pub session_id: Option<String>,
    /// How many times the LLM was called during this turn.
    pub iteration_count: usize,
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

/// Metadata for a session transcript.
#[derive(Clone, Debug, Default)]
pub struct SessionTranscriptMeta {
    pub thread_id: String,
    pub session_id: String,
    pub created_at: Option<String>,
}

/// Session transcript — a parsed conversation record.
#[derive(Clone, Debug, Default)]
pub struct SessionTranscript {
    pub session_id: String,
    pub messages: Vec<TranscriptMessage>,
    pub meta: SessionTranscriptMeta,
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
pub struct PromptContext<'a> {
    pub config: Option<&'a crate::config::Config>,
    pub thread_id: Option<&'a str>,
    /// Learned context data (pre-fetched).
    pub learned: LearnedContextData,
    /// Workspace directory.
    pub workspace_dir: &'a std::path::Path,
    /// Model name.
    pub model_name: &'a str,
    /// Agent identifier.
    pub agent_id: &'a str,
    /// Visible tool names.
    pub visible_tool_names: &'a std::collections::HashSet<String>,
    /// Tool call format.
    pub tool_call_format: ToolCallFormat,
    /// Connected integrations markdown (stub).
    pub connected_identities_md: String,
    /// Whether to include the user profile in the prompt.
    pub include_profile: bool,
    /// Whether to include the memory markdown in the prompt.
    pub include_memory_md: bool,
    /// User identity (stub).
    pub user_identity: Option<String>,
    /// Personality soul markdown.
    pub personality_soul_md: Option<String>,
    /// Personality memory markdown.
    pub personality_memory_md: Option<String>,
    /// Curated snapshot (stub).
    pub curated_snapshot: Option<()>,
    // Slice fields for zero-copy access (stubs use empty slices).
    pub tools: &'a [()],
    pub skills: &'a [()],
    pub dispatcher_instructions: &'a str,
    pub connected_integrations: &'a [()],
    pub personality_roster: Vec<()>,
    pub workflows: &'a [()],
}

/// Context data for learned knowledge injection.
#[derive(Clone, Debug, Default)]
pub struct LearnedContextData {
    pub sections: Vec<String>,
    /// Recent observations from the learning subsystem.
    pub observations: Vec<String>,
    /// Recognized patterns.
    pub patterns: Vec<String>,
    /// Learned user profile entries.
    pub user_profile: Vec<String>,
    /// Explicit user reflections captured from chat.
    pub reflections: Vec<String>,
    /// Pre-fetched root-level summaries from the tree summarizer.
    pub tree_root_summaries: Vec<crate::bridge::agent::NamespaceSummary>,
}

/// A single memory-namespace root summary fetched from the tree summarizer.
#[derive(Clone, Debug)]
pub struct NamespaceSummary {
    /// Memory namespace this root summary belongs to.
    pub namespace: String,
    /// The distilled root summary text.
    pub body: String,
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

    /// Decision payload from a triage run.
    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    pub struct TriageDecision {
        pub action: String,
    }

    /// Outcome of a triage run.
    #[derive(Clone, Debug)]
    pub enum TriageOutcome {
        /// The triage produced a runnable decision.
        Decision(TriageDecision),
        /// The triage was deferred to a later time.
        Deferred { defer_until_ms: u64, reason: String },
    }

    /// Envelope wrapping an inbound composio trigger event.
    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    pub struct TriggerEnvelope {
        pub source: String,
        pub payload: serde_json::Value,
        /// Human-readable display label (e.g. "gmail / message.created").
        pub display_label: String,
        /// Stable external identifier (metadata_id or UUID).
        pub external_id: String,
    }

    impl TriggerEnvelope {
        /// Build a `TriggerEnvelope` from a composio trigger event.
        pub fn from_composio(
            toolkit: &str,
            trigger: &str,
            metadata_id: &str,
            metadata_uuid: &str,
            payload: serde_json::Value,
        ) -> Self {
            Self {
                source: format!("{} / {}", toolkit, trigger),
                display_label: format!("{} / {}", toolkit, trigger),
                external_id: if !metadata_id.is_empty() { metadata_id.to_string() } else { metadata_uuid.to_string() },
                payload,
            }
        }
    }

    pub async fn run_triage(_envelope: &TriggerEnvelope) -> anyhow::Result<TriageOutcome> {
        Err(anyhow::anyhow!("triage not available in standalone mode"))
    }

    pub async fn apply_decision(_decision: TriageDecision, _envelope: &TriggerEnvelope) -> anyhow::Result<()> {
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
