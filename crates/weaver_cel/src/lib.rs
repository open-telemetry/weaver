// SPDX-License-Identifier: Apache-2.0

//! CEL expressions for weaver.
//!
//! The engine only: compile, inspect, evaluate. It has no telemetry types and
//! no weaver dependencies; the crate that owns the samples implements
//! [`Bindings`].

mod bindings;
mod expression;

pub use bindings::Bindings;
pub use expression::{Expression, Referenced};

/// Re-exported so implementors of [`Bindings`] need no direct `cel` dependency.
pub use cel::{Context, Value};

/// Errors from compiling or running an expression.
///
/// Errors identify the expression by its source text; callers should add its
/// origin.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// The expression did not parse.
    #[error("The expression `{expression}` did not compile: {error}")]
    CompileFailed {
        /// Source text of the expression.
        expression: String,
        /// Error from the CEL parser.
        error: String,
    },

    /// The expression failed while running.
    #[error("The expression `{expression}` failed to evaluate: {error}")]
    EvalFailed {
        /// Source text of the expression.
        expression: String,
        /// Error from the CEL interpreter.
        error: String,
    },

    /// The expression returned something other than a bool.
    #[error("The expression `{expression}` returned {value_type}, not a bool")]
    NotBoolean {
        /// Source text of the expression.
        expression: String,
        /// Type the expression returned.
        value_type: String,
    },
}
