// SPDX-License-Identifier: Apache-2.0

//! In-memory logger implementation.
//! Can be used in tests and build.rs scripts.

use std::sync::{Arc, Mutex};

/// An in-memory log message.
#[derive(Debug, Clone)]
pub enum LogMessage {
    /// A trace message.
    Trace(String),
    /// An info message.
    Info(String),
    /// A warning message.
    Warn(String),
    /// An error message.
    Error(String),
    /// A success message.
    Success(String),
    /// A loading message.
    Loading(String),
    /// A log message.
    Log(String),
}

/// A logger that can be used in tests or build.rs scripts.
/// This logger is thread-safe and can be cloned.
#[derive(Default, Clone)]
pub struct Logger {
    messages: Arc<Mutex<Vec<LogMessage>>>,
    debug_level: u8,
}

impl Logger {
    /// Creates a new logger.
    #[must_use]
    pub fn new(debug_level: u8) -> Self {
        Logger {
            messages: Arc::new(Mutex::new(Vec::new())),
            debug_level,
        }
    }

    /// Returns the number of warning messages logged.
    #[must_use]
    pub fn warn_count(&self) -> usize {
        self.messages
            .lock()
            .expect("Failed to lock messages")
            .iter()
            .filter(|m| matches!(m, LogMessage::Warn(_)))
            .count()
    }

    /// Returns the number of error messages logged.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.messages
            .lock()
            .expect("Failed to lock messages")
            .iter()
            .filter(|m| matches!(m, LogMessage::Error(_)))
            .count()
    }

    /// Returns the recorded log messages.
    #[must_use]
    pub fn messages(&self) -> Vec<LogMessage> {
        self.messages
            .lock()
            .expect("Failed to lock messages")
            .clone()
    }
}

impl crate::Logger for Logger {
    /// Logs a trace message (only with debug enabled).
    fn trace(&self, message: &str) {
        if self.debug_level > 0 {
            self.messages
                .lock()
                .expect("Failed to lock messages")
                .push(LogMessage::Trace(message.to_owned()));
        }
    }

    /// Logs an info message.
    fn info(&self, message: &str) {
        self.messages
            .lock()
            .expect("Failed to lock messages")
            .push(LogMessage::Info(message.to_owned()));
    }

    /// Logs a warning message.
    fn warn(&self, message: &str) {
        self.messages
            .lock()
            .expect("Failed to lock messages")
            .push(LogMessage::Warn(message.to_owned()));
    }

    /// Logs an error message.
    fn error(&self, message: &str) {
        self.messages
            .lock()
            .expect("Failed to lock messages")
            .push(LogMessage::Error(message.to_owned()));
    }

    /// Logs a success message.
    fn success(&self, message: &str) {
        self.messages
            .lock()
            .expect("Failed to lock messages")
            .push(LogMessage::Success(message.to_owned()));
    }

    /// Logs a newline.
    fn newline(&self, _count: usize) {}

    /// Indents the logger.
    fn indent(&self, _count: usize) {}

    /// Stops a loading message.
    fn done(&self) {}

    /// Adds a style to the logger.
    fn add_style(&self, _name: &str, _styles: Vec<&'static str>) -> &Self {
        self
    }

    /// Logs a loading message with a spinner.
    fn loading(&self, message: &str) {
        self.messages
            .lock()
            .expect("Failed to lock messages")
            .push(LogMessage::Loading(message.to_owned()));
    }

    /// Forces the logger to not print a newline for the next message.
    fn same(&self) -> &Self {
        self
    }

    /// Logs a message without icon.
    fn log(&self, message: &str) {
        self.messages
            .lock()
            .expect("Failed to lock messages")
            .push(LogMessage::Log(message.to_owned()));
    }
}
