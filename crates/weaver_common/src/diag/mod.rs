// SPDX-License-Identifier: Apache-2.0

//! Defines the diagnostic messages and the corresponding infrastructure used by Weaver.

use std::error::Error;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::thread;
use miette::Diagnostic;

use serde::Serialize;
use serde_json::Value;
use tinytemplate::TinyTemplate;

use crate::diag::channel::DiagChannel;

pub mod channel;
pub mod consumer;

/// A generic diagnostic message.
#[derive(Debug, Serialize)]
pub struct DiagMessage {
    /// The level of the diagnostic message.
    level: DiagLevel,
    /// The diagnostic message. Placeholder values can be used.
    message: String,
    /// The context of the diagnostic message.
    context: Value,
    /// Help information for the diagnostic message.
    help: Option<String>,
    /// Additional notes for the diagnostic message.
    note: Option<String>,
    /// The location of the diagnostic message.
    location: Option<Location>,
}

/// The diagnostic level.
#[derive(Debug, Serialize)]
pub enum DiagLevel {
    /// A warning message.
    Warning,
    /// An error message.
    Error,
}

/// The location of a diagnostic message.
#[derive(Debug, Serialize)]
pub struct Location {
    source: Option<String>,
    line: Option<usize>,
    column: Option<usize>,
}

/// A system message that can be sent to the diagnostic service.
pub enum SystemMessage {
    /// A diagnostic message.
    Diagnostic(DiagMessage),
    /// A stop message used to stop the diagnostic service.
    Stop,
}

/// A diagnostic service that consumes diagnostic messages and reports them to a consumer.
/// The service runs in a separate thread.
pub struct DiagService {
    sender: SyncSender<SystemMessage>,
    join_handle: thread::JoinHandle<()>,
}

impl DiagMessage {
    /// Creates a new diagnostic message with the warning level.
    pub fn warn(message: &str) -> Self {
        Self {
            level: DiagLevel::Warning,
            message: message.to_string(),
            context: Value::Null,
            help: None,
            note: None,
            location: None,
        }
    }

    /// Creates a new diagnostic message with the warning level and a context.
    pub fn warn_with_ctx<C: Serialize>(message: &str, ctx: C) -> Self {
        Self {
            level: DiagLevel::Warning,
            message: message.to_string(),
            // Todo: Fix this
            context: serde_json::to_value(ctx).expect("Failed to serialize context"),
            help: None,
            note: None,
            location: None,
        }
    }

    /// Creates a new diagnostic message with the error level.
    pub fn error(message: &str) -> Self {
        Self {
            level: DiagLevel::Error,
            message: message.to_string(),
            context: Value::Null,
            help: None,
            note: None,
            location: None,
        }
    }
}

impl DiagService {
    /// Creates a new diagnostic service given a [`consumer::DiagMessageConsumer`] and a bound.
    /// The bound is the maximum number of messages that can be buffered.
    /// If the bound is reached, the sender will block until the buffer is freed.
    /// The consumer will consume the messages in the order they were sent.
    /// The service will run in a separate thread.
    ///
    /// The service will stop when the [`DiagService::stop`] method is called.
    pub fn new(consumer: impl consumer::DiagMessageConsumer + 'static, bound: usize) -> Self {
        let (sender, receiver) = sync_channel(bound);
        let join_handle = thread::spawn(move || {
            consumer.run(receiver, |msg| {
                let mut tt = TinyTemplate::new();
                tt.add_template("formatter", &msg.message).expect("Failed to add template");
                tt.render("formatter", &msg.context).expect("Failed to render message")
            });
        });

        Self {
            sender,
            join_handle,
        }
    }

    /// Returns a channel for reporting diagnostic messages.
    pub fn channel(&self) -> DiagChannel {
        DiagChannel::new(self.sender.clone())
    }

    /// Waits for the diagnostic service to finish.
    /// This method should be called at the end of the program.
    /// If this method is not called, the program will hang.
    /// This method should be called only once.
    pub fn stop(self) {
        self.sender.send(SystemMessage::Stop).expect("Failed to send stop message.");
        self.join_handle.join().expect("Failed to join the diagnostic service thread");
    }
}

#[cfg(test)]
mod tests {
    use crate::diag::consumer::console::ConsoleDiagMessageConsumer;

    use super::*;

    #[test]
    fn test_console_diag_service() {
        let consumer = ConsoleDiagMessageConsumer::new(true);
        let service = DiagService::new(consumer, 10);
        let channel = service.channel();

        app_code(&channel);

        service.stop();
    }

    fn app_code(diag_channel: &DiagChannel) {
        let msg = DiagMessage {
            level: DiagLevel::Warning,
            message: "This is a warning message".to_string(),
            context: Value::Null,
            help: None,
            note: None,
            location: None,
        };
        diag_channel.report(DiagMessage::warn("This is a warning message"));
        diag_channel.report(DiagMessage::error("This is an error message"));
    }
}