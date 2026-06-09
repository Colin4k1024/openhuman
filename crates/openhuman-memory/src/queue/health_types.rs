//! Minimal health types for the queue store (avoids depending on tree::health).

/// A pipeline failure classification.
#[derive(Clone, Debug)]
pub struct PipelineFailure {
    pub code: FailureCode,
    pub class: FailureClass,
    pub message: String,
}

impl PipelineFailure {
    pub fn is_unrecoverable(&self) -> bool {
        matches!(self.code, FailureCode::Unrecoverable)
    }
}

impl std::fmt::Display for PipelineFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}:{}] {}", self.code.as_str(), self.class.as_str(), self.message)
    }
}

impl std::error::Error for PipelineFailure {}

/// Failure code classification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureCode {
    Transient,
    ExtractionTimeout,
    EmbeddingFailed,
    Unrecoverable,
}

impl FailureCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Transient => "transient",
            Self::ExtractionTimeout => "extraction_timeout",
            Self::EmbeddingFailed => "embedding_failed",
            Self::Unrecoverable => "unrecoverable",
        }
    }
}

/// Failure class (broad category).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureClass {
    Infrastructure,
    Logic,
    External,
}

impl FailureClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Infrastructure => "infrastructure",
            Self::Logic => "logic",
            Self::External => "external",
        }
    }
}
