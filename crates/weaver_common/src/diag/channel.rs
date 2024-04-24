// SPDX-License-Identifier: Apache-2.0

//! A channel for reporting diagnostic messages.

use std::sync::mpsc::SyncSender;
use crate::diag::{DiagMessage, SystemMessage};

/// A channel for reporting diagnostic messages.
pub struct DiagChannel {
    sender: SyncSender<SystemMessage>,
}

impl DiagChannel {
    /// Create a new diagnostic channel.
    pub(crate) fn new(sender: SyncSender<SystemMessage>) -> Self {
        Self { sender }
    }

    /// Report a diagnostic message.
    pub fn report(&self, message: DiagMessage) {
        self.sender.send(SystemMessage::Diagnostic(message))
            .expect("Failed to send diagnostic message.");
    }
}
