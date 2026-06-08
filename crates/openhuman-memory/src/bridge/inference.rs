//! Inference provider abstraction for LLM-based operations (summarization, extraction).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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

/// A chat message in a conversation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

/// A prompt for a chat completion.
///
/// Supports two construction patterns:
/// 1. System/user convenience: set `system` + `user` + `kind` (legacy tree code)
/// 2. Messages-based: set `messages` + `model` (new code)
#[derive(Clone, Debug, Default)]
pub struct ChatPrompt {
    pub messages: Vec<ChatMessage>,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: Option<u32>,
    /// System prompt content.
    pub system: String,
    /// User message content.
    pub user: String,
    /// Prompt kind label (informational).
    pub kind: &'static str,
}

impl ChatPrompt {
    /// Resolve messages: if `system`/`user` convenience fields are set,
    /// prepend them to `messages`.
    pub fn resolved_messages(&self) -> Vec<ChatMessage> {
        let mut msgs = Vec::new();
        if !self.system.is_empty() {
            msgs.push(ChatMessage::system(&self.system));
        }
        if !self.user.is_empty() {
            msgs.push(ChatMessage::user(&self.user));
        }
        msgs.extend(self.messages.clone());
        msgs
    }
}

/// Usage information from a completion.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub charged_amount_usd: f64,
}

/// Response from a chat completion.
#[derive(Clone, Debug)]
pub struct ChatResponse {
    pub content: String,
    pub usage: Option<UsageInfo>,
}

/// Chat provider trait — the primary inference interface for the memory tree.
#[async_trait]
pub trait ChatProvider: Send + Sync {
    /// Provider name for logging.
    fn name(&self) -> &str;

    /// Complete a chat prompt.
    async fn complete_chat(&self, prompt: &ChatPrompt) -> anyhow::Result<ChatResponse>;

    /// Simple system+user completion (convenience method used by tree_runtime).
    async fn chat_with_system(
        &self,
        system_prompt: Option<&str>,
        user_message: &str,
        model: &str,
        temperature: f64,
    ) -> anyhow::Result<String> {
        let prompt = ChatPrompt {
            system: system_prompt.unwrap_or_default().to_string(),
            user: user_message.to_string(),
            model: model.to_string(),
            temperature,
            kind: "chat_with_system",
            ..Default::default()
        };
        let resp = self.complete_chat(&prompt).await?;
        Ok(resp.content)
    }

    /// Chat expecting JSON output (used by entity extraction).
    async fn chat_for_json(&self, prompt: &ChatPrompt) -> anyhow::Result<String> {
        let resp = self.complete_chat(prompt).await?;
        Ok(resp.content)
    }

    /// Chat with message history (used by query/walk).
    async fn chat_with_history(
        &self,
        messages: &[ChatMessage],
        model: &str,
        temperature: f64,
    ) -> anyhow::Result<String> {
        let prompt = ChatPrompt {
            messages: messages.to_vec(),
            model: model.to_string(),
            temperature,
            ..Default::default()
        };
        let resp = self.complete_chat(&prompt).await?;
        Ok(resp.content)
    }

    /// Chat returning text + usage info (used by summarise).
    async fn chat_for_text_with_usage(
        &self,
        prompt: &ChatPrompt,
    ) -> anyhow::Result<(String, Option<UsageInfo>)> {
        let resp = self.complete_chat(prompt).await?;
        Ok((resp.content, resp.usage))
    }
}

/// A static chat provider that always returns the same content (for tests).
pub struct StaticChatProvider {
    response: String,
}

impl StaticChatProvider {
    pub fn new(response: impl Into<String>) -> Self {
        Self { response: response.into() }
    }
}

#[async_trait]
impl ChatProvider for StaticChatProvider {
    fn name(&self) -> &str { "static" }

    async fn complete_chat(&self, _prompt: &ChatPrompt) -> anyhow::Result<ChatResponse> {
        Ok(ChatResponse {
            content: self.response.clone(),
            usage: None,
        })
    }
}

/// Alias for the ChatProvider trait (used in code that references `Provider`).
pub use ChatProvider as Provider;

/// Build a chat runtime from config (placeholder — full app provides real impl).
pub fn build_chat_runtime(
    _config: &crate::config::Config,
) -> anyhow::Result<(Box<dyn ChatProvider>, String)> {
    anyhow::bail!("chat runtime not configured in standalone mode")
}

/// Build a chat provider (placeholder — full app provides real impl).
pub fn build_chat_provider(
    _config: &crate::config::Config,
) -> anyhow::Result<Box<dyn ChatProvider>> {
    anyhow::bail!("chat provider not configured in standalone mode")
}

/// Test override for chat provider (tests only).
#[cfg(test)]
pub fn test_override(_provider: Box<dyn ChatProvider>) {
    // No-op in standalone.
}
