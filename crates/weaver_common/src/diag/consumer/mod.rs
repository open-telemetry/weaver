// SPDX-License-Identifier: Apache-2.0

//! A consumer that consumes diagnostic messages.

pub mod console;

use crate::diag::SystemMessage;
use std::sync::mpsc;

/// A consumer of diagnostic messages.
pub trait DiagMessageConsumer: Send {
    /// Runs the consumer.
    /// The consumer is expected to consume diagnostic messages from the given receiver, report
    /// them, and stop when the `SystemMessage::Stop` message is received.
    fn run(&self, receiver: mpsc::Receiver<SystemMessage>);
}
