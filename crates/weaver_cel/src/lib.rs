// SPDX-License-Identifier: Apache-2.0

//! CEL expressions for weaver.
//!
//! Compilation, inspection and evaluation only: no telemetry types and no
//! weaver dependencies. The crate that owns the samples implements
//! [`Bindings`].

mod bindings;
mod expression;
mod free_variables;
mod matches;

pub use bindings::Bindings;
pub use expression::{Expression, Referenced, Scope};

/// Re-exported so implementors of [`Bindings`] need no direct `cel` dependency.
pub use cel::{Context, Value};

/// Errors from compiling or running an expression.
///
/// Each variant carries the expression source text; the caller adds its
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

    /// A literal `matches` pattern is not a valid regex.
    #[error("The expression `{expression}` has an invalid `matches` pattern `{pattern}`: {error}")]
    BadPattern {
        /// Source text of the expression.
        expression: String,
        /// The pattern that did not compile.
        pattern: String,
        /// Error from the regex parser.
        error: String,
    },

    /// The expression failed during evaluation.
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
