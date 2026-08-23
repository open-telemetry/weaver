// SPDX-License-Identifier: Apache-2.0

//! CEL experiments for live-check matchers.
//!
//! A matcher gives live-check an identifier for the samples that do not carry
//! one, by testing a CEL expression against the sample. This crate is a
//! self-contained playground for those expressions: a minimal sample model, a
//! matcher config, and the glue that binds one to the other.

pub mod matcher;
pub mod sample;

/// Errors raised while compiling or running a matcher expression.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// The expression did not parse.
    #[error("The `when` expression of matcher `{matcher_id}` did not compile: {error}")]
    CompileFailed {
        /// The id of the matcher.
        matcher_id: String,
        /// The error reported by the CEL parser.
        error: String,
    },

    /// The expression failed while running against a sample.
    #[error("The `when` expression of matcher `{matcher_id}` failed to evaluate: {error}")]
    EvalFailed {
        /// The id of the matcher.
        matcher_id: String,
        /// The error reported by the CEL interpreter.
        error: String,
    },

    /// The expression came out as something other than a boolean.
    #[error("The `when` expression of matcher `{matcher_id}` returned {value_type}, not a bool")]
    NotBoolean {
        /// The id of the matcher.
        matcher_id: String,
        /// The type the expression returned.
        value_type: String,
    },
}
