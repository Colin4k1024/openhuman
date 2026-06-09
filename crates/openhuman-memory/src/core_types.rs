//! Core transport types (mirrors src/core/ types for standalone compilation).

use serde::Serialize;
use serde_json::{Map, Value};
use std::future::Future;
use std::pin::Pin;

/// Future returned by controller handlers.
pub type ControllerFuture = Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'static>>;

/// Handler function type.
pub type ControllerHandler = fn(Map<String, Value>) -> ControllerFuture;

/// A registered controller entry.
pub struct RegisteredController {
    pub schema: ControllerSchema,
    pub handler: ControllerHandler,
}

impl RegisteredController {
    pub fn rpc_method_name(&self) -> String {
        format!("openhuman.{}_{}", self.schema.namespace, self.schema.function)
    }
}

/// Schema for a controller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ControllerSchema {
    pub namespace: &'static str,
    pub function: &'static str,
    pub description: &'static str,
    pub inputs: Vec<FieldSchema>,
    pub outputs: Vec<FieldSchema>,
}

impl ControllerSchema {
    pub fn method_name(&self) -> String {
        format!("{}.{}", self.namespace, self.function)
    }
}

/// Schema for a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldSchema {
    pub name: &'static str,
    pub ty: TypeSchema,
    pub comment: &'static str,
    pub required: bool,
}

/// Schema for a type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum TypeSchema {
    Bool,
    I64,
    U64,
    F64,
    String,
    Json,
    Bytes,
    Array(Box<TypeSchema>),
    Map(Box<TypeSchema>),
    Option(Box<TypeSchema>),
    Enum { variants: Vec<&'static str> },
    Object { fields: Vec<FieldSchema> },
    Ref(&'static str),
}

/// Namespace for controller registration.
pub mod all {
    pub use super::{ControllerFuture, ControllerHandler, RegisteredController};
}

/// Logging helpers (stubs).
pub mod logging {
    #[derive(Clone, Debug)]
    pub enum CliLogDefault {
        Global,
    }

    pub fn init_for_cli_run(_verbose: bool, _default: CliLogDefault) {}
}

/// Shutdown helpers (stubs).
pub mod shutdown {
    pub fn register<F, Fut>(_f: F)
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
    }
}

/// Observability helpers (stubs).
pub mod observability {
    /// Report an error with context tags.
    /// Signature mirrors the real app: (error, domain, operation, tags).
    pub fn report_error(
        _err: impl std::fmt::Display,
        _domain: &str,
        _operation: &str,
        _tags: &[(&str, &str)],
    ) {}
    pub fn report_error_or_expected(
        _msg: &str,
        _domain: &str,
        _operation: &str,
        _tags: &[(&str, &str)],
    ) {}
}

/// Event bus compat (remaining references).
pub mod event_bus {
    pub use crate::bridge::events::*;
}
