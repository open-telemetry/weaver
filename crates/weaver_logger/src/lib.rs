// SPDX-License-Identifier: Apache-2.0

//! A generic logger that can be used to log messages to the console.

pub mod quiet;

use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

/// A trait that defines the interface of a logger.
pub trait Logger {
    /// Logs an trace message (only with debug enabled).
    fn trace(&self, message: &str);

    /// Logs an info message.
    fn info(&self, message: &str);

    /// Logs a warning message.
    fn warn(&self, message: &str);

    /// Logs an error message.
    fn error(&self, message: &str);

    /// Logs a success message.
    fn success(&self, message: &str);

    /// Logs a newline.
    fn newline(&self, count: usize);

    /// Indents the logger.
    fn indent(&self, count: usize);

    /// Stops a loading message.
    fn done(&self);

    /// Adds a style to the logger.
    fn add_style(&self, name: &str, styles: Vec<&'static str>) -> &Self;

    /// Logs a loading message with a spinner.
    fn loading(&self, message: &str);

    /// Forces the logger to not print a newline for the next message.
    fn same(&self) -> &Self;

    /// Logs a message without icon.
    fn log(&self, message: &str);
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
    #[must_use]
    pub fn new(debug_level: u8) -> Self {
        ConsoleLogger {
            logger: Arc::new(Mutex::new(paris::Logger::new())),
            debug_level,
        }
    }
}

impl Logger for ConsoleLogger {
    /// Logs an trace message (only with debug enabled).
    fn trace(&self, message: &str) {
        if self.debug_level > 0 {
            _ = self
                .logger
                .lock()
                .expect("Failed to lock logger")
                .log(message);
        }
    }

    /// Logs an info message.
    fn info(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .info(message);
    }

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
    fn success(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .success(message);
    }

    /// Logs a newline.
    fn newline(&self, count: usize) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .newline(count);
    }

    /// Indents the logger.
    fn indent(&self, count: usize) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .indent(count);
    }

    /// Stops a loading message.
    fn done(&self) {
        _ = self.logger.lock().expect("Failed to lock logger").done();
    }

    /// Adds a style to the logger.
    fn add_style(&self, name: &str, styles: Vec<&'static str>) -> &Self {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .add_style(name, styles);
        self
    }

    /// Logs a loading message with a spinner.
    fn loading(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .loading(message);
    }

    /// Forces the logger to not print a newline for the next message.
    fn same(&self) -> &Self {
        _ = self.logger.lock().expect("Failed to lock logger").same();
        self
    }

    /// Logs a message without icon.
    fn log(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .log(message);
    }
}

/// A logger that does not log anything.
#[derive(Default, Clone)]
pub struct NullLogger {}

impl NullLogger {
    /// Creates a new logger.
    #[must_use]
    pub fn new() -> Self {
        NullLogger {}
    }
}

impl Logger for NullLogger {
    /// Logs an trace message (only with debug enabled).
    fn trace(&self, _: &str) {}

    /// Logs an info message.
    fn info(&self, _: &str) {}

    /// Logs a warning message.
    fn warn(&self, _: &str) {}

    /// Logs an error message.
    fn error(&self, _: &str) {}

    /// Logs a success message.
    fn success(&self, _: &str) {}

    /// Logs a newline.
    fn newline(&self, _: usize) {}

    /// Indents the logger.
    fn indent(&self, _: usize) {}

    /// Stops a loading message.
    fn done(&self) {}

    /// Adds a style to the logger.
    fn add_style(&self, _: &str, _: Vec<&'static str>) -> &Self {
        self
    }

    /// Logs a loading message with a spinner.
    fn loading(&self, _: &str) {}

    /// Forces the logger to not print a newline for the next message.
    fn same(&self) -> &Self {
        self
    }

    /// Logs a message without icon.
    fn log(&self, _: &str) {}
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
    #[must_use]
    pub fn new() -> Self {
        TestLogger {
            logger: Arc::new(Mutex::new(paris::Logger::new())),
            warn_count: Arc::new(AtomicUsize::new(0)),
            error_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the number of warning messages logged.
    #[must_use]
    pub fn warn_count(&self) -> usize {
        self.warn_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Returns the number of error messages logged.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.error_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Logger for TestLogger {
    /// Logs a trace message (only with debug enabled).
    fn trace(&self, _message: &str) {}

    /// Logs an info message.
    fn info(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .info(message);
    }

    /// Logs a warning message.
    fn warn(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .warn(message);
        _ = self
            .warn_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Logs an error message.
    fn error(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .error(message);
        _ = self
            .error_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Logs a success message.
    fn success(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .success(message);
    }

    /// Logs a newline.
    fn newline(&self, count: usize) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .newline(count);
    }

    /// Indents the logger.
    fn indent(&self, count: usize) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .indent(count);
    }

    /// Stops a loading message.
    fn done(&self) {
        _ = self.logger.lock().expect("Failed to lock logger").done();
    }

    /// Adds a style to the logger.
    fn add_style(&self, name: &str, styles: Vec<&'static str>) -> &Self {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .add_style(name, styles);
        self
    }

    /// Logs a loading message with a spinner.
    fn loading(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .loading(message);
    }

    /// Forces the logger to not print a newline for the next message.
    fn same(&self) -> &Self {
        _ = self.logger.lock().expect("Failed to lock logger").same();
        self
    }

    /// Logs a message without icon.
    fn log(&self, message: &str) {
        _ = self
            .logger
            .lock()
            .expect("Failed to lock logger")
            .log(message);
    }
}
