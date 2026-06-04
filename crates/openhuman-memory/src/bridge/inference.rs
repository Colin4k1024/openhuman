//! Inference provider abstraction for LLM-based operations (summarization, extraction).

use async_trait::async_trait;

/// Minimal inference interface needed by the memory system.
#[async_trait]
pub trait InferenceProvider: Send + Sync {
    /// Simple one-shot completion with a system prompt and user message.
    async fn complete(
        &self,
        system_prompt: &str,
        user_message: &str,
        model: &str,
        temperature: f64,
    ) -> anyhow::Result<String>;
}
