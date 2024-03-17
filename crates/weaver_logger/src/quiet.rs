// SPDX-License-Identifier: Apache-2.0

//! Logger in quiet mode.
//! This logger only logs errors and warnings.

use crate::Logger;
use std::sync::{Arc, Mutex};

/// A quient logger that can be used to log messages to the console.
/// This logger is thread-safe and can be cloned.
/// Only logs errors and warnings are logged to the console.
#[derive(Default, Clone)]
pub struct QuietLogger {
    logger: Arc<Mutex<paris::Logger<'static>>>,
}

impl QuietLogger {
    /// Creates a new logger.
    #[must_use]
    pub fn new() -> Self {
        Self {
            logger: Arc::new(Mutex::new(paris::Logger::new())),
        }
    }
}

impl Logger for QuietLogger {
    /// Logs a trace message (only with debug enabled).
    fn trace(&self, _message: &str) {}

    /// Logs an info message.
    fn info(&self, _message: &str) {}

    /// Logs a warning message.
    fn warn(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .warn(message);
    }

    /// Logs an error message.
    fn error(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .error(message);
    }

    /// Logs a success message.
    fn success(&self, _message: &str) {}

    /// Logs a newline.
    fn newline(&self, _count: usize) {}

    /// Indents the logger.
    fn indent(&self, _count: usize) {}

    /// Stops a loading message.
    fn done(&self) {
        _ = self.logger.lock().expect("Failed to lock logger").done();
    }

    /// Adds a style to the logger.
    fn add_style(&self, _name: &str, _styles: Vec<&'static str>) -> &Self {
        self
    }

    /// Logs a loading message with a spinner.
    fn loading(&self, _message: &str) {}

    /// Forces the logger to not print a newline for the next message.
    fn same(&self) -> &Self {
        self
    }

    /// Logs a message without icon.
    fn log(&self, _message: &str) {}
}
