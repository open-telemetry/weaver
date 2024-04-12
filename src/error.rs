// SPDX-License-Identifier: Apache-2.0

//! Error management

use std::fmt::Display;
use std::process::exit;

// Define a trait with the exit_if_error method
pub trait ExitIfError<T, E> {
    /// Call the error code and exit the process if the result is an error.
    /// Otherwise, return the value.
    ///
    /// # Arguments
    /// * `self` - The result to check
    /// * `err_handler` - The error handler to call if the result is an error
    ///
    /// # Returns
    /// The value if the result is Ok
    fn exit_if_error<F: FnOnce(E)>(self, err_handler: F) -> T;

    /// Call the error code and exit the process with the given code if the
    /// result is an error.
    ///
    /// # Arguments
    /// * `self` - The result to check
    /// * `code` - The exit code to use if the result is an error
    /// * `err_handler` - The error handler to call if the result is an error
    ///
    /// # Returns
    /// The value if the result is Ok
    #[allow(dead_code)]
    fn exit_with_code_if_error<F: FnOnce(E)>(self, code: i32, err_handler: F) -> T;
}

// Implement the trait for all Result<T, E> where E is an error.
impl<T, E: Display> ExitIfError<T, E> for Result<T, E> {
    /// Call the error code and exit the process if the result is an error.
    /// Otherwise, return the value.
    ///
    /// # Arguments
    /// * `self` - The result to check
    /// * `err_handler` - The error handler to call if the result is an error
    ///
    /// # Returns
    /// The value if the result is Ok
    fn exit_if_error<F: FnOnce(E)>(self, err_handler: F) -> T {
        match self {
            Ok(value) => value,
            Err(e) => {
                err_handler(e);
                #[allow(clippy::exit)] // Expected behavior
                exit(1)
            }
        }
    }

    /// Call the error code and exit the process with the given code if the
    /// result is an error.
    ///
    /// # Arguments
    /// * `self` - The result to check
    /// * `code` - The exit code to use if the result is an error
    /// * `err_handler` - The error handler to call if the result is an error
    ///
    /// # Returns
    /// The value if the result is Ok
    fn exit_with_code_if_error<F: FnOnce(E)>(self, code: i32, err_handler: F) -> T {
        match self {
            Ok(value) => value,
            Err(e) => {
                err_handler(e);
                #[allow(clippy::exit)] // Expected behavior
                exit(code)
            }
        }
    }
}
