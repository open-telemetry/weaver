// SPDX-License-Identifier: Apache-2.0

//! Render examples

use crate::search::theme::ThemeConfig;
use ratatui::prelude::{Line, Span, Style};
use weaver_semconv::attribute::ExamplesSpec;

/// Append examples to the text.
pub fn append_lines(examples: &ExamplesSpec, text: &mut Vec<Line>, theme: &ThemeConfig) {
    text.push(Line::from(Span::styled(
        "Examples: ",
        Style::default().fg(theme.label),
    )));
    match examples {
        ExamplesSpec::Int(v) => text.push(Line::from(Span::raw(format!("- {}", v)))),
        ExamplesSpec::Double(v) => text.push(Line::from(Span::raw(format!("- {}", v)))),
        ExamplesSpec::Bool(v) => text.push(Line::from(Span::raw(format!("- {}", v)))),
        ExamplesSpec::String(v) => text.push(Line::from(Span::raw(format!("- {}", v)))),
        ExamplesSpec::Ints(vals) => {
            for v in vals.iter() {
                text.push(Line::from(Span::raw(format!("- {}", v))));
            }
        }
        ExamplesSpec::Doubles(vals) => {
            for v in vals.iter() {
                text.push(Line::from(Span::raw(format!("- {}", v))));
            }
        }
        ExamplesSpec::Bools(vals) => {
            for v in vals.iter() {
                text.push(Line::from(Span::raw(format!("- {}", v))));
            }
        }
        ExamplesSpec::Strings(vals) => {
            for v in vals.iter() {
                text.push(Line::from(Span::raw(format!("- {}", v))));
            }
        }
    }
}
