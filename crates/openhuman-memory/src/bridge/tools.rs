//! Tool trait abstraction.
//!
//! Replaces `crate::openhuman::tools::traits::{Tool, ToolResult}`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Result from a tool execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub metadata: Option<Value>,
}

impl ToolResult {
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            metadata: None,
        }
    }

    pub fn err(output: impl Into<String>) -> Self {
        Self {
            success: false,
            output: output.into(),
            metadata: None,
        }
    }
}

/// Permission level for a tool.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PermissionLevel {
    #[default]
    ReadOnly,
    Write,
    Admin,
}

/// Tool category for grouping.
#[derive(Clone, Debug, Default)]
pub enum ToolCategory {
    #[default]
    General,
    Memory,
    System,
    Network,
}

/// Trait all agent tools implement.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool identifier.
    fn name(&self) -> &str;

    /// Human-readable description.
    fn description(&self) -> &str;

    /// JSON Schema for the tool's parameters.
    fn parameters_schema(&self) -> Value;

    /// Execute the tool with the given arguments.
    async fn execute(&self, args: Value) -> ToolResult;

    /// Permission level required.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    /// Whether this tool can be called concurrently.
    fn is_concurrency_safe(&self) -> bool {
        true
    }

    /// Tool category.
    fn category(&self) -> ToolCategory {
        ToolCategory::General
    }
}
