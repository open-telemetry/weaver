// SPDX-License-Identifier: Apache-2.0

//! Render semantic convention attributes.

use ratatui::prelude::{Line, Span, Style};
use ratatui::widgets::Paragraph;

use crate::search::theme::ThemeConfig;
use weaver_semconv::MetricSpecWithProvenance;

use crate::search::semconv::attributes;

#[cfg(not(tarpaulin_include))]
pub fn widget<'a>(
    metric: Option<&'a MetricSpecWithProvenance>,
    theme: &'a ThemeConfig,
) -> Paragraph<'a> {
    match metric {
        Some(MetricSpecWithProvenance { metric, provenance }) => {
            let mut text = vec![
                Line::from(vec![
                    Span::styled("Name      : ", Style::default().fg(theme.label)),
                    Span::raw(metric.name.clone()),
                ]),
                Line::from(vec![
                    Span::styled("Instrument: ", Style::default().fg(theme.label)),
                    Span::raw(format!("{:?}", metric.instrument)),
                ]),
                Line::from(vec![
                    Span::styled("Unit      : ", Style::default().fg(theme.label)),
                    Span::raw(metric.unit.clone().unwrap_or_default()),
                ]),
            ];

            // Brief
            if !metric.brief.trim().is_empty() {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    "Brief     : ",
                    Style::default().fg(theme.label),
                )));
                text.push(Line::from(metric.brief.as_str()));
            }

            // Note
            if !metric.note.trim().is_empty() {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    "Note      : ",
                    Style::default().fg(theme.label),
                )));
                text.push(Line::from(metric.note.as_str()));
            }

            attributes::append_lines(metric.attributes.as_slice(), &mut text, theme);

            // Provenance
            text.push(Line::from(""));
            text.push(Line::from(vec![
                Span::styled("Provenance: ", Style::default().fg(theme.label)),
                Span::raw(provenance.to_string()),
            ]));

            Paragraph::new(text).style(Style::default().fg(theme.value))
        }
        None => Paragraph::new(vec![Line::default()]),
    }
}
