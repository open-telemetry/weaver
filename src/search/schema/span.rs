// SPDX-License-Identifier: Apache-2.0

//! Utility functions to index and render spans.

use crate::search::schema::{attribute, attributes, tags};
use crate::search::theme::ThemeConfig;
use crate::search::DocFields;
use ratatui::prelude::{Line, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use tantivy::{doc, IndexWriter};
use weaver_schema::TelemetrySchema;

/// Build index for spans.
pub fn index(schema: &TelemetrySchema, fields: &DocFields, index_writer: &mut IndexWriter) {
    for span in schema.spans() {
        let tags: String = span.tags.clone().map_or("".to_string(), |tags| {
            tags.iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        });

        index_writer
            .add_document(doc!(
                fields.path => format!("schema/span/{}", span.span_name),
                fields.brief => "",
                fields.note => "",
                fields.tag => tags.as_str(),
            ))
            .expect("Failed to add document");
        attribute::index_schema_attribute(
            span.attributes.iter(),
            &format!("schema/span/{}", span.span_name),
            fields,
            index_writer,
        );
        for event in span.events.iter() {
            let tags: String = event.tags.clone().map_or("".to_string(), |tags| {
                tags.iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ")
            });

            index_writer
                .add_document(doc!(
                    fields.path => format!("schema/span/{}/event/{}", span.span_name, event.event_name),
                    fields.brief => "",
                    fields.note => "",
                    fields.tag => tags.as_str(),
                ))
                .expect("Failed to add document");
            attribute::index_schema_attribute(
                event.attributes.iter(),
                &format!("schema/span/{}/event/{}", span.span_name, event.event_name),
                fields,
                index_writer,
            );
        }
    }
}

/// Render a span details.
pub fn widget<'a>(
    span: Option<&'a weaver_schema::span::Span>,
    provenance: &'a str,
    theme: &'a ThemeConfig,
) -> Paragraph<'a> {
    match span {
        Some(span) => {
            let mut text = vec![
                Line::from(vec![
                    Span::styled("Type      : ", Style::default().fg(theme.label)),
                    Span::raw("Span (schema)"),
                ]),
                Line::from(vec![
                    Span::styled("Name      : ", Style::default().fg(theme.label)),
                    Span::raw(&span.span_name),
                ]),
            ];

            if let Some(kind) = span.kind.as_ref() {
                text.push(Line::from(vec![
                    Span::styled("Kind      : ", Style::default().fg(theme.label)),
                    Span::raw(format!("{:?}", kind)),
                ]));
            }

            attributes::append_lines(span.attributes.as_slice(), &mut text, theme);

            if !span.events.is_empty() {
                text.push(Line::from(Span::styled(
                    "Events    : ",
                    Style::default().fg(theme.label),
                )));
                for event in span.events.iter() {
                    text.push(Line::from(Span::raw(format!("- {} ", event.event_name))));
                }
            }

            if !span.links.is_empty() {
                text.push(Line::from(Span::styled(
                    "Links     : ",
                    Style::default().fg(theme.label),
                )));
                for link in span.links.iter() {
                    text.push(Line::from(Span::raw(format!("- {} ", link.link_name))));
                }
            }

            tags::append_lines(span.tags.as_ref(), &mut text, theme);

            // Provenance
            text.push(Line::from(""));
            text.push(Line::from(Span::styled(
                "Provenance: ",
                Style::default().fg(theme.label),
            )));
            text.push(Line::from(provenance));

            Paragraph::new(text).style(Style::default().fg(theme.value))
        }
        None => Paragraph::new(vec![Line::default()]),
    }
}
