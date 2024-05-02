// SPDX-License-Identifier: Apache-2.0

//! A generic trait for errors that can be returned by the weaver crates.
//! This trait is used by the logging infrastructure to print errors in
//! a consistent way.

use crate::Logger;
use std::process::exit;

/// A trait for custom error handling in the `weaver` crates.
pub trait WeaverError<T> {
    /// Retrieves a list of error messages associated with this error.
    /// For compound errors, this method should return a list of all
    /// error messages. For simple errors, this method should return
    /// a list with a single error message.
    ///
    /// # Returns
    /// A `Vec<String>` containing human-readable error messages.
    fn errors(&self) -> Vec<String>;

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

/// A trait for types that can cleanly exit the application if an error
/// is encountered.
pub trait ExitIfError<T, E> {
    /// Processes the `Result` and panics if it is an `Err`.
    /// If `Ok`, the contained value is returned.
    ///
    /// # Arguments
    /// * `self` - The `Result` to process.
    /// * `logger` - An object implementing the `Logger` trait used to log any
    /// errors.
    ///
    /// # Returns
    /// The contained value if the result is `Ok`.
    /// Panics if the result is `Err`.
    fn panic_if_error(self, logger: impl Logger) -> T;

    /// Processes the `Result` and exits the application if it is an `Err`.
    /// If `Ok`, the contained value is returned.
    ///
    /// # Arguments
    /// * `self` - The `Result` to process.
    /// * `logger` - An object implementing the `Logger` trait used to log any
    /// errors.
    ///
    /// # Returns
    /// The contained value if the result is `Ok`.
    /// Exits the process if the result is `Err`.
    fn exit_if_error(self, logger: impl Logger) -> T;

    /// Processes the `Result` and exits the application with a specified exit
    /// code if it is an `Err`.
    /// If `Ok`, the contained value is returned.
    ///
    /// # Arguments
    /// * `self` - The `Result` to process.
    /// * `code` - The exit code to use if the result is an `Err`.
    /// * `logger` - An object implementing the `Logger` trait used to log any
    /// errors.
    ///
    /// # Returns
    /// The contained value if the result is `Ok`.
    /// Exits the process with the specified `code` if the result is `Err`.
    #[allow(dead_code)]
    fn exit_with_code_if_error(self, code: i32, logger: impl Logger) -> T;
}

/// Provides default implementations of the `ExitIfError` trait for any
/// `Result<T, E>` where `E` implements `WeaverError`.
impl<T, E: WeaverError<E>> ExitIfError<T, E> for Result<T, E> {
    /// Processes the `Result` and panics if it is an `Err`.
    /// If `Ok`, the contained value is returned.
    ///
    /// # Arguments
    /// * `self` - The `Result` to process.
    /// * `logger` - An object implementing the `Logger` trait used to log any
    /// errors.
    ///
    /// # Returns
    /// The contained value if the result is `Ok`.
    /// Panics if the result is `Err`.
    fn panic_if_error(self, logger: impl Logger) -> T {
        match self {
            Ok(value) => value,
            Err(e) => {
                e.errors().iter().for_each(|msg| logger.error(msg));
                panic!("One or several errors occurred (see above)");
            }
        }
    }

    /// Processes the `Result` and exits the application if it is an `Err`.
    /// If `Ok`, the contained value is returned.
    ///
    /// # Arguments
    /// * `self` - The `Result` to process.
    /// * `logger` - An object implementing the `Logger` trait used to log any
    /// errors.
    ///
    /// # Returns
    /// The contained value if the result is `Ok`.
    /// Exits the process if the result is `Err`.
    fn exit_if_error(self, logger: impl Logger) -> T {
        match self {
            Ok(value) => value,
            Err(e) => {
                e.errors().iter().for_each(|msg| logger.error(msg));
                #[allow(clippy::exit)] // Expected behavior
                exit(1)
            }
        }
    }

    /// Processes the `Result` and exits the application with a specified exit
    /// code if it is an `Err`.
    /// If `Ok`, the contained value is returned.
    ///
    /// # Arguments
    /// * `self` - The `Result` to process.
    /// * `code` - The exit code to use if the result is an `Err`.
    /// * `logger` - An object implementing the `Logger` trait used to log any
    /// errors.
    ///
    /// # Returns
    /// The contained value if the result is `Ok`.
    /// Exits the process with the specified `code` if the result is `Err`.
    fn exit_with_code_if_error(self, code: i32, logger: impl Logger) -> T {
        match self {
            Ok(value) => value,
            Err(e) => {
                e.errors().iter().for_each(|msg| logger.error(msg));
                #[allow(clippy::exit)] // Expected behavior
                exit(code)
            }
        }
    }
}
