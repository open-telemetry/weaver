// SPDX-License-Identifier: Apache-2.0

//!

// use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::Serialize;
use weaver_common::diag::channel::DiagChannel;
use weaver_common::diag::consumer::console::ConsoleDiagMessageConsumer;
use weaver_common::diag::{DiagMessage, DiagService};

// #[derive(Debug, Diagnostic)]
// #[diagnostic(
// help("try doing it better next time?")
// )]
// struct MyBad {
//     test: String,
// }

fn main() {
    // let my_bad = MyBad { test: "test".to_string() };
    // println!("{:?}", my_bad.code());
    // println!("{:?}", my_bad.help());

    let consumer = ConsoleDiagMessageConsumer::new(true);
    let service = DiagService::new(consumer, 10);
    let channel = service.channel();

    app_code(&channel);

    service.stop();
}

fn app_code(diag_channel: &DiagChannel) {
    #[derive(Serialize)]
    struct Test {
        field: String,
    }

    diag_channel.report(DiagMessage::warn_with_ctx("This is a warning message (field: {field})", Test { field: "value".to_string() }));
    diag_channel.report(DiagMessage::error("This is an error message"));
}