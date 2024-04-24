// SPDX-License-Identifier: Apache-2.0

//! A channel for reporting diagnostic messages.

use crate::diag::SystemMessage;
use miette::Diagnostic;
use std::error::Error;
use std::sync::mpsc::SyncSender;

/// A channel for reporting diagnostic messages.
#[derive(Clone)]
pub struct DiagChannel {
    sender: SyncSender<SystemMessage>,
}

impl DiagChannel {
    /// Create a new diagnostic channel.
    pub(crate) fn new(sender: SyncSender<SystemMessage>) -> Self {
        Self { sender }
    }

    /// Report a diagnostic message.
    pub fn report<M: Error + Diagnostic + Send + Sync + 'static>(&self, message: M) {
        // JUSTIFICATION: The only way this can fail is if the receiver has been dropped.
        self.sender.send(SystemMessage::Diagnostic(message.into()))
            .expect("The DiagService has been stopped while the application is still running and generating diagnostic messages. Please ensure that the DiagService is stopped only after the rest of the application has finished.");
    }
}
