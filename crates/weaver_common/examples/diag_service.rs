// SPDX-License-Identifier: Apache-2.0

//!

use crate::DiagnosticMessages::{MyAdvice, MyError, MyMessage, MyWarning};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;
use weaver_common::diag::channel::DiagChannel;
use weaver_common::diag::consumer::console::ConsoleDiagMessageConsumer;
use weaver_common::diag::DiagService;

#[derive(Error, Diagnostic, Debug)]
enum DiagnosticMessages {
    #[error("A fantastic diagnostic error!")]
    #[diagnostic(
        code(oops::my::bad),
        severity(Error),
        url(docsrs),
        help("try doing it better next time?")
    )]
    MyError {
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
    MyAdvice {
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
    MyWarning {
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
    MyMessage {
        // The Source that we're gonna be printing snippets out of.
        // This can be a String if you don't have or care about file names.
        #[source_code]
        src: NamedSource<String>,
        // Snippets and highlights can be included in the diagnostic!
        #[label("This bit here")]
        bad_bit: SourceSpan,
    },
}

fn main() {
    let consumer = ConsoleDiagMessageConsumer::new(true);
    let service = DiagService::new(consumer, 10);
    let channel = service.channel();

    app_code(&channel);

    service.stop();
}

fn app_code(diag_channel: &DiagChannel) {
    let src = "source\n  text\n    here".to_string();

    diag_channel.report(MyError {
        src: NamedSource::new("bad_file.rs", src.clone()),
        bad_bit: (9, 4).into(),
    });
    diag_channel.report(MyAdvice {
        src: NamedSource::new("bad_file.rs", src.clone()),
        bad_bit: (9, 4).into(),
    });
    diag_channel.report(MyWarning {
        src: NamedSource::new("bad_file.rs", src.clone()),
        bad_bit: (9, 4).into(),
    });
    diag_channel.report(MyMessage {
        src: NamedSource::new("bad_file.rs", src),
        bad_bit: (9, 4).into(),
    });
}
