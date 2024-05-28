// SPDX-License-Identifier: Apache-2.0

//! A generic trait for errors that can be returned by the weaver crates.
//! This trait is used by the logging infrastructure to print errors in
//! a consistent way.

use miette::Diagnostic;
use serde::Serialize;

/// A trait marker for Weaver diagnostic.
pub trait WeaverDiagnostic {}

/// A blanket implementation of the `WeaverDiagnostic` trait for any type that
/// implements the `Diagnostic` and `Serialize` traits.
///
/// This allows any type that implements `Diagnostic` and `Serialize` to be
/// converted into [crate::diagnostic::DiagnosticMessages].
impl<T> WeaverDiagnostic for T where T: Serialize + Diagnostic + Send + Sync + ?Sized {}

/// A trait for custom error handling in the `weaver` crates.
pub trait WeaverError<T>: Serialize + Diagnostic + Send + Sync {
    /// Constructs a single compound error from a collection of individuals.
    #[must_use]
    fn compound(errors: Vec<T>) -> T;
}

/// Handles a list of errors and returns a compound error if the list is not
/// empty or () if the list is empty.
pub fn handle_errors<T: WeaverError<T>>(mut errors: Vec<T>) -> Result<(), T> {
    if errors.is_empty() {
        Ok(())
    } else if errors.len() == 1 {
        Err(errors
            .pop()
            .expect("should never happen as we checked the length"))
    } else {
        Err(T::compound(errors))
    }
}

/// Formats the given errors into a single string.
/// This used to render compound errors.
#[must_use]
pub fn format_errors<E: WeaverError<E> + std::fmt::Display>(errors: &[E]) -> String {
    errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<String>>()
        .join("\n\n")
}
