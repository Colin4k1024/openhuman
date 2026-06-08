//! Core transport types (minimal stubs for standalone compilation).
//!
//! In the full app, these are provided by `src/core/types.rs` and `src/core/all.rs`.

use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

/// Future returned by controller handlers.
pub type ControllerFuture =
    Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;

/// A registered controller entry.
pub struct RegisteredController {
    pub namespace: &'static str,
    pub name: &'static str,
    pub handler: fn(Value) -> ControllerFuture,
}

/// Schema for a controller.
#[derive(Clone, Debug)]
pub struct ControllerSchema {
    pub namespace: String,
    pub name: String,
    pub description: String,
    pub fields: Vec<FieldSchema>,
}

/// Schema for a field.
#[derive(Clone, Debug)]
pub struct FieldSchema {
    pub name: String,
    pub description: String,
    pub type_schema: TypeSchema,
    pub required: bool,
}

/// Schema for a type.
#[derive(Clone, Debug)]
pub enum TypeSchema {
    String,
    Number,
    Boolean,
    Object,
    Array(Box<TypeSchema>),
    Optional(Box<TypeSchema>),
}

/// Namespace for controller registration.
pub mod all {
    pub use super::{ControllerFuture, RegisteredController};
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
    pub fn register(_f: impl FnOnce() + Send + 'static) {}
}

/// Observability helpers (stubs).
pub mod observability {
    pub fn report_error(_msg: &str) {}
}
