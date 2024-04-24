// SPDX-License-Identifier: Apache-2.0

//! A consumer that writes diagnostic messages to the console as plain text.

#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

use crate::diag::consumer::DiagMessageConsumer;
use crate::diag::SystemMessage;
use std::sync::mpsc;

/// A consumer that writes diagnostic messages to the console as plain text.
pub struct ConsoleDiagMessageConsumer {
    stdout_lock: bool,
}

impl ConsoleDiagMessageConsumer {
    /// Creates a new console consumer.
    #[must_use]
    pub fn new(stdout_lock: bool) -> Self {
        Self { stdout_lock }
    }
}

impl DiagMessageConsumer for ConsoleDiagMessageConsumer {
    /// Runs the console consumer.
    /// The consumer is expected to consume diagnostic messages from the given receiver, report
    /// them, and handle the `SystemMessage::Stop` message.
    fn run(&self, receiver: mpsc::Receiver<SystemMessage>) {
        let stdout = std::io::stdout();
        let lock = if self.stdout_lock {
            // Used to speed up the output to the console.
            Some(stdout.lock())
        } else {
            None
        };

        for message in receiver {
            match message {
                SystemMessage::Diagnostic(report) => {
                    if let Some(severity) = report.severity() {
                        match severity {
                            miette::Severity::Advice => {
                                println!("Advice: {:?}", report);
                            }
                            miette::Severity::Warning => {
                                eprintln!("Warning: {:?}", report);
                            }
                            miette::Severity::Error => {
                                eprintln!("Error: {:?}", report);
                            }
                        }
                    } else {
                        println!("{:?}", report);
                    }
                }
                SystemMessage::Stop => {
                    break;
                }
            }
        }
        drop(lock);
    }
}
