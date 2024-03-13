// SPDX-License-Identifier: Apache-2.0

//! A generic logger that can be used to log messages to the console.

#![deny(missing_docs)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]

pub mod quiet;

use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

/// A trait that defines the interface of a logger.
pub trait Logger {
    /// Logs an trace message (only with debug enabled).
    fn trace(&self, message: &str) -> &Self;

    /// Logs an info message.
    fn info(&self, message: &str) -> &Self;

    /// Logs a warning message.
    fn warn(&self, message: &str) -> &Self;

    /// Logs an error message.
    fn error(&self, message: &str) -> &Self;

    /// Logs a success message.
    fn success(&self, message: &str) -> &Self;

    /// Logs a newline.
    fn newline(&self, count: usize) -> &Self;

    /// Indents the logger.
    fn indent(&self, count: usize) -> &Self;

    /// Stops a loading message.
    fn done(&self);

    /// Adds a style to the logger.
    fn add_style(&self, name: &str, styles: Vec<&'static str>) -> &Self;

    /// Logs a loading message with a spinner.
    fn loading(&self, message: &str) -> &Self;

    /// Forces the logger to not print a newline for the next message.
    fn same(&self) -> &Self;

    /// Logs a message without icon.
    fn log(&self, message: &str) -> &Self;
}

/// A generic logger that can be used to log messages to the console.
/// This logger is thread-safe and can be cloned.
#[derive(Default, Clone)]
pub struct ConsoleLogger {
    logger: Arc<Mutex<paris::Logger<'static>>>,
    debug_level: u8,
}

impl ConsoleLogger {
    /// Creates a new logger.
    pub fn new(debug_level: u8) -> Self {
        ConsoleLogger {
            logger: Arc::new(Mutex::new(paris::Logger::new())),
            debug_level,
        }
    }
}

impl Logger for ConsoleLogger {
    /// Logs an trace message (only with debug enabled).
    fn trace(&self, message: &str) -> &Self {
        if self.debug_level > 0 {
            self.logger
                .lock()
                .expect("Failed to lock logger")
                .log(message);
        }
        self
    }

    /// Logs an info message.
    fn info(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .info(message);
        self
    }

    /// Logs a warning message.
    fn warn(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .warn(message);
        self
    }

    /// Logs an error message.
    fn error(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .error(message);
        self
    }

    /// Logs a success message.
    fn success(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .success(message);
        self
    }

    /// Logs a newline.
    fn newline(&self, count: usize) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .newline(count);
        self
    }

    /// Indents the logger.
    fn indent(&self, count: usize) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .indent(count);
        self
    }

    /// Stops a loading message.
    fn done(&self) {
        self.logger.lock().expect("Failed to lock logger").done();
    }

    /// Adds a style to the logger.
    fn add_style(&self, name: &str, styles: Vec<&'static str>) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .add_style(name, styles);
        self
    }

    /// Logs a loading message with a spinner.
    fn loading(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .loading(message);
        self
    }

    /// Forces the logger to not print a newline for the next message.
    fn same(&self) -> &Self {
        self.logger.lock().expect("Failed to lock logger").same();
        self
    }

    /// Logs a message without icon.
    fn log(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .log(message);
        self
    }
}

/// A logger that does not log anything.
#[derive(Default, Clone)]
pub struct NullLogger {}

impl NullLogger {
    /// Creates a new logger.
    pub fn new() -> Self {
        NullLogger {}
    }
}

impl Logger for NullLogger {
    /// Logs an trace message (only with debug enabled).
    fn trace(&self, _: &str) -> &Self {
        self
    }

    /// Logs an info message.
    fn info(&self, _: &str) -> &Self {
        self
    }

    /// Logs a warning message.
    fn warn(&self, _: &str) -> &Self {
        self
    }

    /// Logs an error message.
    fn error(&self, _: &str) -> &Self {
        self
    }

    /// Logs a success message.
    fn success(&self, _: &str) -> &Self {
        self
    }

    /// Logs a newline.
    fn newline(&self, _: usize) -> &Self {
        self
    }

    /// Indents the logger.
    fn indent(&self, _: usize) -> &Self {
        self
    }

    /// Stops a loading message.
    fn done(&self) {}

    /// Adds a style to the logger.
    fn add_style(&self, _: &str, _: Vec<&'static str>) -> &Self {
        self
    }

    /// Logs a loading message with a spinner.
    fn loading(&self, _: &str) -> &Self {
        self
    }

    /// Forces the logger to not print a newline for the next message.
    fn same(&self) -> &Self {
        self
    }

    /// Logs a message without icon.
    fn log(&self, _: &str) -> &Self {
        self
    }
}

/// A logger that can be used in unit or integration tests.
/// This logger is thread-safe and can be cloned.
#[derive(Default, Clone)]
pub struct TestLogger {
    logger: Arc<Mutex<paris::Logger<'static>>>,
    warn_count: Arc<AtomicUsize>,
    error_count: Arc<AtomicUsize>,
}

impl TestLogger {
    /// Creates a new logger.
    pub fn new() -> Self {
        TestLogger {
            logger: Arc::new(Mutex::new(paris::Logger::new())),
            warn_count: Arc::new(AtomicUsize::new(0)),
            error_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the number of warning messages logged.
    pub fn warn_count(&self) -> usize {
        self.warn_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Returns the number of error messages logged.
    pub fn error_count(&self) -> usize {
        self.error_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Logger for TestLogger {
    /// Logs a trace message (only with debug enabled).
    fn trace(&self, _message: &str) -> &Self {
        self
    }

    /// Logs an info message.
    fn info(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .info(message);
        self
    }

    /// Logs a warning message.
    fn warn(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .warn(message);
        self.warn_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self
    }

    /// Logs an error message.
    fn error(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .error(message);
        self.error_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self
    }

    /// Logs a success message.
    fn success(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .success(message);
        self
    }

    /// Logs a newline.
    fn newline(&self, count: usize) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .newline(count);
        self
    }

    /// Indents the logger.
    fn indent(&self, count: usize) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .indent(count);
        self
    }

    /// Stops a loading message.
    fn done(&self) {
        self.logger.lock().expect("Failed to lock logger").done();
    }

    /// Adds a style to the logger.
    fn add_style(&self, name: &str, styles: Vec<&'static str>) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .add_style(name, styles);
        self
    }

    /// Logs a loading message with a spinner.
    fn loading(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .loading(message);
        self
    }

    /// Forces the logger to not print a newline for the next message.
    fn same(&self) -> &Self {
        self.logger.lock().expect("Failed to lock logger").same();
        self
    }

    /// Logs a message without icon.
    fn log(&self, message: &str) -> &Self {
        self.logger
            .lock()
            .expect("Failed to lock logger")
            .log(message);
        self
    }
}
