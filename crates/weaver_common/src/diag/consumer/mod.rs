// SPDX-License-Identifier: Apache-2.0

//!

pub mod console;
pub mod json;

use std::sync::mpsc;
use crate::diag::{DiagMessage, SystemMessage};

/// A consumer of diagnostic messages.
pub trait DiagMessageConsumer: Send {
    /// Runs the consumer.
    /// The consumer is expected to consume diagnostic messages from the given receiver, report
    /// them, and handle the `SystemMessage::Stop` message.
    fn run(&self, receiver: mpsc::Receiver<SystemMessage>, msg_formatter: fn(&DiagMessage) -> String);
}