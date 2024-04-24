// SPDX-License-Identifier: Apache-2.0

//!

use std::error::Error;
use miette::{Diagnostic, Report, SourceSpan};
use miette::{NamedSource, Result};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("oops!")]
#[diagnostic(
code(oops::my::bad),
severity(Error),
url(docsrs),
help("try doing it better next time?")
)]
struct MyDiag {
    // The Source that we're gonna be printing snippets out of.
    // This can be a String if you don't have or care about file names.
    #[source_code]
    src: NamedSource<String>,
    // Snippets and highlights can be included in the diagnostic!
    #[label("This bit here")]
    bad_bit: SourceSpan,
}

/// Todo
pub enum SystemMessage {
    /// A diagnostic message.
    Diagnostic(Report),
    /// A stop message used to stop the diagnostic service.
    Stop,
}

/// Todo
pub struct Channel {
    msgs: Vec<SystemMessage>,
}

impl Channel {
    /// Todo
    pub fn report<E: Error + Diagnostic + Send + Sync + 'static>(&mut self, diag: E) {
        self.msgs.push(SystemMessage::Diagnostic(diag.into()));
    }
}

fn main() -> Result<()> {
    let mut channel = Channel { msgs: Vec::new() };

    let src = "source\n  text\n    here".to_string();
    channel.report(MyDiag {
        src: NamedSource::new("bad_file.rs", src),
        bad_bit: (9, 4).into(),
    });

    let src = "source\n  text\n    here".to_string();
    channel.report(MyDiag {
        src: NamedSource::new("bad_file2.rs", src),
        bad_bit: (9, 4).into(),
    });

    for msg in channel.msgs {
        match msg {
            SystemMessage::Diagnostic(report) => {
                println!("{:?}", report);
            },
            SystemMessage::Stop => {
                println!("Stopping");
            },
        }
    }

    Ok(())
}

// fn report<E: Error + Diagnostic + Send + Sync + 'static>(diag: E) {
//     let report: Report = diag.into();
//
//     let sys_msg = SystemMessage::Diagnostic(report);
//
//     match sys_msg {
//         SystemMessage::Diagnostic(report) => {
//             println!("{:?}", report);
//         },
//         SystemMessage::Stop => {
//             println!("Stopping");
//         },
//     }
// }