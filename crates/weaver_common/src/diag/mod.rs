// SPDX-License-Identifier: Apache-2.0

//! Diagnostic infrastructure used to report diagnostic messages to the user.
//! The diagnostic messages are based on the [`miette`] crate.
//! This infrastructure is designed to be extensible and flexible.

use std::sync::mpsc::{sync_channel, SyncSender};
use std::thread;

use miette::Report;

use crate::diag::channel::DiagChannel;

pub mod channel;
pub mod consumer;

/// A system message that can be sent to the diagnostic service.
pub enum SystemMessage {
    /// A diagnostic report.
    Diagnostic(Report),
    /// A stop message used to stop the diagnostic service.
    Stop,
}

/// A diagnostic service that consumes diagnostic messages and reports them to a consumer.
/// The service runs in a separate thread.
pub struct DiagService {
    sender: SyncSender<SystemMessage>,
    join_handle: thread::JoinHandle<()>,
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
            consumer.run(receiver);
        });

        Self {
            sender,
            join_handle,
        }
    }

    /// Returns a channel for reporting diagnostic reports.
    #[must_use]
    pub fn channel(&self) -> DiagChannel {
        DiagChannel::new(self.sender.clone())
    }

    /// Waits for the diagnostic service to finish.
    /// This method should be called at the end of the program.
    /// If this method is not called, the program will hang.
    /// This method should be called only once.
    pub fn stop(self) {
        self.sender
            .send(SystemMessage::Stop)
            .expect("The DiagService has already been stopped.");
        self.join_handle
            .join()
            .expect("The DiagService thread has panicked.");
    }
}

#[cfg(test)]
mod tests {
    use miette::{Diagnostic, NamedSource, SourceSpan};
    use thiserror::Error;

    use crate::diag::consumer::console::ConsoleDiagMessageConsumer;

    use super::*;

    #[derive(Error, Diagnostic, Debug)]
    enum DiagMessages {
        #[error("A fantastic diagnostic error!")]
        #[diagnostic(
            code(oops::my::bad),
            severity(Error),
            url(docsrs),
            help("try doing it better next time?")
        )]
        Error {
            // The Source that we're gonna be printing snippets out of.
            // This can be a String if you don't have or care about file names.
            #[source_code]
            src: NamedSource<String>,
            // Snippets and highlights can be included in the diagnostic!
            #[label("This bit here")]
            bad_bit: SourceSpan,
        },

        #[error("A fantastic diagnostic advice!")]
        #[diagnostic(
            code(oops::my::bad),
            severity(Advice),
            url(docsrs),
            help("try doing it better next time?")
        )]
        Advice {
            // The Source that we're gonna be printing snippets out of.
            // This can be a String if you don't have or care about file names.
            #[source_code]
            src: NamedSource<String>,
            // Snippets and highlights can be included in the diagnostic!
            #[label("This bit here")]
            bad_bit: SourceSpan,
        },

        #[error("A fantastic diagnostic warning!")]
        #[diagnostic(
            code(oops::my::bad),
            severity(Warning),
            url(docsrs),
            help("try doing it better next time?")
        )]
        Warning {
            // The Source that we're gonna be printing snippets out of.
            // This can be a String if you don't have or care about file names.
            #[source_code]
            src: NamedSource<String>,
            // Snippets and highlights can be included in the diagnostic!
            #[label("This bit here")]
            bad_bit: SourceSpan,
        },

        #[error("A fantastic diagnostic message!")]
        #[diagnostic(
            code(oops::my::bad),
            url(docsrs),
            help("try doing it better next time?")
        )]
        Message {
            // The Source that we're gonna be printing snippets out of.
            // This can be a String if you don't have or care about file names.
            #[source_code]
            src: NamedSource<String>,
            // Snippets and highlights can be included in the diagnostic!
            #[label("This bit here")]
            bad_bit: SourceSpan,
        },
        #[error("Compound errors")]
        CompoundError {
            #[related]
            errors: Vec<DiagMessages>,
        }
    }

    #[test]
    fn test_console_diag_service() {
        let consumer = ConsoleDiagMessageConsumer::new(true);
        let service = DiagService::new(consumer, 10);
        let channel = service.channel();

        single_thread_app(&channel);
        multi_thread_app(&channel);

        service.stop();
    }

    #[test]
    fn test_console_diag_service_without_stdout_lock() {
        let consumer = ConsoleDiagMessageConsumer::new(false);
        let service = DiagService::new(consumer, 10);
        let channel = service.channel();

        single_thread_app(&channel);
        multi_thread_app(&channel);

        service.stop();
    }

    /// This function represent a single threaded application that reports a diagnostic message.
    fn single_thread_app(diag_channel: &DiagChannel) {
        let src = "source\n  text\n    here".to_owned();

        diag_channel.report(DiagMessages::Error {
            src: NamedSource::new("bad_file.rs", src.clone()),
            bad_bit: (9, 4).into(),
        });
        diag_channel.report(DiagMessages::Advice {
            src: NamedSource::new("bad_file.rs", src.clone()),
            bad_bit: (9, 4).into(),
        });
        diag_channel.report(DiagMessages::Warning {
            src: NamedSource::new("bad_file.rs", src.clone()),
            bad_bit: (9, 4).into(),
        });
        diag_channel.report(DiagMessages::Message {
            src: NamedSource::new("bad_file.rs", src),
            bad_bit: (9, 4).into(),
        });
    }

    /// This function represent a multithreaded application that reports a diagnostic message.
    /// Note that the Rust compiler will force you to clone the `DiagChannel` to pass it to the
    /// thread.
    fn multi_thread_app(diag_channel: &DiagChannel) {
        let diag_channel = diag_channel.clone();

        _ = thread::spawn(move || {
            let src = "source\n  text\n    here".to_owned();

            diag_channel.report(DiagMessages::Error {
                src: NamedSource::new("bad_file.rs", src.clone()),
                bad_bit: (9, 4).into(),
            });
            diag_channel.report(DiagMessages::Advice {
                src: NamedSource::new("bad_file.rs", src.clone()),
                bad_bit: (9, 4).into(),
            });
            diag_channel.report(DiagMessages::Warning {
                src: NamedSource::new("bad_file.rs", src.clone()),
                bad_bit: (9, 4).into(),
            });
            diag_channel.report(DiagMessages::Message {
                src: NamedSource::new("bad_file.rs", src),
                bad_bit: (9, 4).into(),
            });
        })
        .join();
    }
}
