// SPDX-License-Identifier: Apache-2.0

//! Utility functions to index and render attributes.

use crate::search::schema::tags;
use crate::search::semconv::examples;
use crate::search::theme::ThemeConfig;
use crate::search::DocFields;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use tantivy::{doc, IndexWriter};
use weaver_schema::attribute::Attribute;

/// Build index for semantic convention attributes.
pub fn index_semconv_attributes<'a>(
    attributes: impl Iterator<Item = &'a weaver_semconv::attribute::AttributeSpec>,
    path: &str,
    fields: &DocFields,
    index_writer: &mut IndexWriter,
) {
    for attr in attributes {
        _ = index_writer
            .add_document(doc!(
                fields.path => format!("{}/attr/{}", path, attr.id()),
                fields.brief => attr.brief(),
                fields.note => attr.note(),
                fields.tag => attr.tag().unwrap_or_default().as_str(),
            ))
            .expect("Failed to add document");
    }
}

/// Build index for schema attributes.
pub fn index_schema_attribute<'a>(
    attributes: impl Iterator<Item = &'a Attribute>,
    path: &str,
    fields: &DocFields,
    index_writer: &mut IndexWriter,
) {
    for attr in attributes {
        if let Attribute::Id {
            id,
            brief,
            note,
            tags,
            ..
        } = attr
        {
            let tags: String = tags.as_ref().map_or("".to_owned(), |tags| {
                tags.iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ")
            });

            _ = index_writer
                .add_document(doc!(
                    fields.path => format!("{}/attr/{}", path, id),
                    fields.brief => brief.clone(),
                    fields.note => note.clone(),
                    fields.tag => tags.as_str(),
                ))
                .expect("Failed to add document");
        }
    }
}

/// Render an attribute details.
pub fn widget<'a>(
    attribute: Option<&'a Attribute>,
    provenance: &'a str,
    theme: &ThemeConfig,
) -> Paragraph<'a> {
    match attribute {
        Some(Attribute::Id {
            id,
            r#type,
            brief,
            examples,
            tag,
            requirement_level,
            sampling_relevant,
            note,
            stability,
            deprecated,
            tags,
            value,
        }) => {
            let mut text = vec![
                Line::from(vec![
                    Span::styled("Id   : ", Style::default().fg(theme.label)),
                    Span::raw(id),
                ]),
                Line::from(vec![
                    Span::styled("Type : ", Style::default().fg(theme.label)),
                    Span::raw(r#type.to_string()),
                ]),
            ];

            // Tag
            if let Some(tag) = tag {
                text.push(Line::from(vec![
                    Span::styled("Tag  : ", Style::default().fg(theme.label)),
                    Span::raw(tag),
                ]));
            }

            // Brief
            if !brief.trim().is_empty() {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    "Brief: ",
                    Style::default().fg(theme.label),
                )));
                text.push(Line::from(brief.as_str()));
            }

            // Note
            if !note.trim().is_empty() {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    "Note : ",
                    Style::default().fg(theme.label),
                )));
                text.push(Line::from(note.as_str()));
            }

            // Requirement Level
            text.push(Line::from(""));
            text.push(Line::from(vec![
                Span::styled("Requirement Level: ", Style::default().fg(theme.label)),
                Span::raw(format!("{}", requirement_level)),
            ]));

            if let Some(sampling_relevant) = sampling_relevant {
                text.push(Line::from(vec![
                    Span::styled("Sampling Relevant: ", Style::default().fg(theme.label)),
                    Span::raw(sampling_relevant.to_string()),
                ]));
            }

            if let Some(stability) = stability {
                text.push(Line::from(vec![
                    Span::styled("Stability: ", Style::default().fg(theme.label)),
                    Span::raw(format!("{}", stability)),
                ]));
            }

            if let Some(deprecated) = deprecated {
                text.push(Line::from(vec![
                    Span::styled("Deprecated: ", Style::default().fg(theme.label)),
                    Span::raw(deprecated.to_string()),
                ]));
            }

            if let Some(examples) = examples {
                examples::append_lines(examples, &mut text, theme);
            }

            if let Some(value) = value {
                text.push(Line::from(vec![
                    Span::styled("Value: ", Style::default().fg(theme.label)),
                    Span::raw(format!("{}", value)),
                ]));
            }

            tags::append_lines(tags.as_ref(), &mut text, theme);

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
        _ => Paragraph::new(vec![Line::from("Attribute not resolved!")]),
    }
}
