// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

pub mod diagnostic;
pub mod error;
pub mod result;
pub mod test;
pub mod vdir;

use crate::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use crate::error::{format_errors, WeaverError};
use crate::Error::CompoundError;
use miette::Diagnostic;
use paris::formatter::colorize_string;
use serde::Serialize;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// All the errors emitted by this crate.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Home directory not found.
    #[error("Home directory not found")]
    HomeDirNotFound,

    /// Cache directory not created.
    #[error("Cache directory not created: {message}")]
    CacheDirNotCreated {
        /// The error message
        message: String,
    },

    /// Git repo not created.
    #[error("Git repo `{repo_url}` not created: {message}")]
    GitRepoNotCreated {
        /// The git repo URL
        repo_url: String,
        /// The error message
        message: String,
    },

    /// A git error occurred.
    #[error("Git error occurred while cloning `{repo_url}`: {message}")]
    GitError {
        /// The git repo URL
        repo_url: String,
        /// The error message
        message: String,
    },

    /// An invalid registry path.
    #[error("The registry path `{path}` is invalid: {error}")]
    InvalidRegistryPath {
        /// The registry path
        path: String,
        /// The error message
        error: String,
    },

    /// A virtual directory error.
    #[error("Virtual directory `{path}` is invalid: {error}")]
    InvalidVirtualDirectory {
        /// The virtual directory path
        path: String,
        /// The error message
        error: String,
    },

    /// An invalid registry archive.
    #[error("The registry archive `{archive}` is invalid: {error}")]
    InvalidRegistryArchive {
        /// The registry archive path
        archive: String,
        /// The error message
        error: String,
    },

    /// An invalid registry archive.
    #[error("This archive `{archive}` is not supported. Supported formats are: .tar.gz, .zip")]
    UnsupportedRegistryArchive {
        /// The registry archive path
        archive: String,
    },

    /// A container for multiple errors.
    #[error("{:?}", format_errors(.0))]
    CompoundError(#[related] Vec<Error>),
}

impl WeaverError<Error> for Error {
    fn compound(errors: Vec<Error>) -> Error {
        CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    CompoundError(errors) => errors,
                    e => vec![e],
                })
                .collect(),
        )
    }
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(match error {
            CompoundError(errors) => errors
                .into_iter()
                .flat_map(|e| {
                    let diag_msgs: DiagnosticMessages = e.into();
                    diag_msgs.into_inner()
                })
                .collect(),
            _ => vec![DiagnosticMessage::new(error)],
        })
    }
}

/// A logger implementation for the standard Rust `log` crate that can be used in tests.
/// This logger tracks warning and error counts and is thread-safe.
#[derive(Default, Clone)]
pub struct TestLog {
    warn_count: Arc<AtomicUsize>,
    error_count: Arc<AtomicUsize>,
}

impl TestLog {
    /// Creates a new test logger.
    #[must_use]
    pub fn new() -> Self {
        TestLog {
            warn_count: Arc::new(AtomicUsize::new(0)),
            error_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the number of warning messages logged.
    #[must_use]
    pub fn warn_count(&self) -> usize {
        self.warn_count.load(Ordering::Relaxed)
    }

    /// Returns the number of error messages logged.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Registers this logger as the global logger.
    ///
    /// # Errors
    ///
    /// Returns an error if setting the logger fails.
    pub fn init(self) -> Result<(), log::SetLoggerError> {
        log::set_max_level(log::LevelFilter::Trace);
        log::set_boxed_logger(Box::new(self))
    }
}

impl log::Log for TestLog {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        match record.level() {
            log::Level::Warn => {
                _ = self.warn_count.fetch_add(1, Ordering::Relaxed);
            }
            log::Level::Error => {
                _ = self.error_count.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        // Print the log message to stderr
        std::io::stderr()
            .write_fmt(format_args!("{} - {}\n", record.level(), record.args()))
            .expect("Failed to write log message");
    }

    fn flush(&self) {}
}

/// A stored log record with all relevant information
#[derive(Debug, Clone)]
pub struct StoredRecord {
    /// The level of the log record
    pub level: log::Level,
    /// The target of the log record
    pub target: String,
    /// The message of the log record
    pub message: String,
}

/// A logger implementation for the standard Rust `log` crate that stores records in memory.
/// This logger keeps all logs in a vector for later inspection and is thread-safe.
#[derive(Default, Clone)]
pub struct MemLog {
    records: Arc<Mutex<Vec<StoredRecord>>>,
}

impl MemLog {
    /// Creates a new memory logger.
    #[must_use]
    pub fn new() -> Self {
        MemLog {
            records: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Returns all stored log records.
    #[must_use]
    pub fn records(&self) -> Vec<StoredRecord> {
        self.records.lock().expect("Failed to lock records").clone()
    }

    /// Returns the number of warning messages logged.
    #[must_use]
    pub fn warn_count(&self) -> usize {
        self.records
            .lock()
            .expect("Failed to lock records")
            .iter()
            .filter(|record| record.level == log::Level::Warn)
            .count()
    }

    /// Returns the number of error messages logged.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.records
            .lock()
            .expect("Failed to lock records")
            .iter()
            .filter(|record| record.level == log::Level::Error)
            .count()
    }

    /// Registers this logger as the global logger.
    ///
    /// # Errors
    ///
    /// Returns an error if setting the logger fails.
    pub fn init(self) -> Result<(), log::SetLoggerError> {
        log::set_max_level(log::LevelFilter::Trace);
        log::set_boxed_logger(Box::new(self))
    }
}

impl log::Log for MemLog {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        let stored_record = StoredRecord {
            level: record.level(),
            target: record.target().to_owned(),
            message: record.args().to_string(),
        };

        self.records
            .lock()
            .expect("Failed to lock records")
            .push(stored_record);
    }

    fn flush(&self) {}
}

/// Adds some success flare to text
pub fn success_flare<T: std::fmt::Display>(message: T) -> String {
    colorize_string(format!("<green><tick></> {}", message))
}
/// Logs a success message as info
pub fn log_success<T: std::fmt::Display>(message: T) {
    log::info!("{}", success_flare(message));
}

/// Adds some error flare to text
pub fn error_flare<T: std::fmt::Display>(message: T) -> String {
    colorize_string(format!("<red><cross></> {}", message))
}
/// Logs an error message as error
pub fn log_error<T: std::fmt::Display>(message: T) {
    log::error!("{}", error_flare(message));
}

/// Adds some info flare to text
pub fn info_flare<T: std::fmt::Display>(message: T) -> String {
    colorize_string(format!("<cyan><info></> {}", message))
}
/// Logs an info message as info
pub fn log_info<T: std::fmt::Display>(message: T) {
    log::info!("{}", info_flare(message));
}

/// Adds some warning flare to text
pub fn warn_flare<T: std::fmt::Display>(message: T) -> String {
    colorize_string(format!("<yellow><warn></> {}", message))
}
/// Logs a warning message as warn
pub fn log_warn<T: std::fmt::Display>(message: T) {
    log::warn!("{}", warn_flare(message));
}
