// SPDX-License-Identifier: Apache-2.0

//! Utility functions to index and render metric groups.

use ratatui::prelude::{Line, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use tantivy::{doc, IndexWriter};

use weaver_schema::metric_group::{Metric, MetricGroup};
use weaver_schema::TelemetrySchema;

use crate::search::schema::{attributes, tags};
use crate::search::theme::ThemeConfig;
use crate::search::DocFields;

/// Build index for metrics.
pub fn index(schema: &TelemetrySchema, fields: &DocFields, index_writer: &mut IndexWriter) {
    for metric_group in schema.metric_groups() {
        let tags: String = metric_group.tags().map_or("".to_string(), |tags| {
            tags.iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        });

        _ = index_writer
            .add_document(doc!(
                fields.path => format!("schema/metric_group/{}", metric_group.name()),
                fields.brief => "",
                fields.note => "",
                fields.tag => tags.as_str(),
            ))
            .expect("Failed to add document");
    }
}

/// Render a metric details.
pub fn widget<'a>(
    metric_group: Option<&'a MetricGroup>,
    provenance: &'a str,
    theme: &'a ThemeConfig,
) -> Paragraph<'a> {
    match metric_group {
        Some(metric_group) => {
            let mut text = vec![Line::from(vec![
                Span::styled("Type      : ", Style::default().fg(theme.label)),
                Span::raw("Metric Group (schema)"),
            ])];

            text.push(Line::from(vec![
                Span::styled("Name      : ", Style::default().fg(theme.label)),
                Span::raw(metric_group.name.clone()),
            ]));

            attributes::append_lines(metric_group.attributes.as_slice(), &mut text, theme);

            if !metric_group.metrics.is_empty() {
                text.push(Line::from(Span::styled(
                    "Metrics   : ",
                    Style::default().fg(theme.label),
                )));
                for metric in metric_group.metrics.iter() {
                    if let Metric::Metric {
                        name,
                        instrument,
                        unit,
                        tags,
                        ..
                    } = metric
                    {
                        let mut properties = vec![];
                        properties.push(format!("instrument={:?}", instrument));
                        if let Some(unit) = unit {
                            properties.push(format!("unit={}", unit));
                        }
                        if let Some(tags) = tags {
                            if !tags.is_empty() {
                                let mut pairs = vec![];
                                for (k, v) in tags.iter() {
                                    pairs.push(format!("{}={}", k, v));
                                }
                                properties.push(format!("tags=[{}]", pairs.join(",")));
                            }
                        }
                        let properties = if properties.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", properties.join(", "))
                        };
                        text.push(Line::from(Span::raw(format!("- {}{}", name, properties))));
                    }
                }
            }

            tags::append_lines(metric_group.tags.as_ref(), &mut text, theme);

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
