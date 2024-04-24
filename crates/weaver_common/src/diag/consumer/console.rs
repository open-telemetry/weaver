// SPDX-License-Identifier: Apache-2.0

//! A consumer that writes diagnostic messages to the console as plain text.

use std::sync::mpsc;
use crate::diag::consumer::DiagMessageConsumer;
use crate::diag::{DiagMessage, SystemMessage};

/// A consumer that writes diagnostic messages to the console as plain text.
pub struct ConsoleDiagMessageConsumer {
    stdout_lock: bool,
}

impl ConsoleDiagMessageConsumer {
    /// Creates a new console consumer.
    pub fn new(stdout_lock: bool) -> Self {
        Self {
            stdout_lock,
        }
    }
}

impl DiagMessageConsumer for ConsoleDiagMessageConsumer {
    /// Runs the console consumer.
    /// The consumer is expected to consume diagnostic messages from the given receiver, report
    /// them, and handle the `SystemMessage::Stop` message.
    fn run(&self, receiver: mpsc::Receiver<SystemMessage>, msg_formatter: fn(&DiagMessage) -> String) {
        let lock = if self.stdout_lock {
            Some(std::io::stdout().lock())
        } else {
            None
        };

        for message in receiver {
            match message {
                SystemMessage::Diagnostic(message) => {
                    let level = match message.level {
                        crate::diag::DiagLevel::Warning => "warning",
                        crate::diag::DiagLevel::Error => "error",
                    };

                    let location = match message.location {
                        Some(ref location) => {
                            let source = match location.source.as_ref() {
                                Some(source) => format!("{}:", source),
                                None => "".to_string(),
                            };

                            let line = match location.line {
                                Some(line) => format!("{}:", line),
                                None => "".to_string(),
                            };

                            let column = match location.column {
                                Some(column) => format!("{}:", column),
                                None => "".to_string(),
                            };

                            format!("{}{}{}", source, line, column)
                        }
                        None => "".to_string(),
                    };

                    let help = match message.help.as_ref() {
                        Some(help) => format!("help: {}\n", help),
                        None => "".to_string(),
                    };

                    let note = match message.note.as_ref() {
                        Some(note) => format!("note: {}\n", note),
                        None => "".to_string(),
                    };

                    println!("{}: {}{}{}{}", level, location, msg_formatter(&message), help, note);
                }
                SystemMessage::Stop => {
                    break;
                }
            }
        }
        drop(lock);
    }
}