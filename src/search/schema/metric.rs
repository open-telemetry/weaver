// SPDX-License-Identifier: Apache-2.0

//! Utility functions to index and render metrics.

use ratatui::prelude::{Line, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use tantivy::{doc, IndexWriter};

use weaver_schema::univariate_metric::UnivariateMetric;
use weaver_schema::TelemetrySchema;

use crate::search::schema::{attribute, attributes, tags};
use crate::search::theme::ThemeConfig;
use crate::search::DocFields;

/// Build index for semantic convention metrics.
pub fn index_semconv_metrics<'a>(
    metrics: impl Iterator<Item = &'a weaver_semconv::metric::MetricSpec>,
    path: &str,
    fields: &DocFields,
    index_writer: &mut IndexWriter,
) {
    for metric in metrics {
        _ = index_writer
            .add_document(doc!(
                fields.path => format!("{}/metric/{}", path, metric.name),
                fields.brief => metric.brief(),
                fields.note => metric.note(),
                fields.tag => "",
            ))
            .expect("Failed to add document");
    }
}

/// Build index for schema metrics.
pub fn index_schema_metrics(
    schema: &TelemetrySchema,
    fields: &DocFields,
    index_writer: &mut IndexWriter,
) {
    for metric in schema.metrics() {
        let tags: String = metric.tags().map_or("".to_string(), |tags| {
            tags.iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        });

        _ = index_writer
            .add_document(doc!(
                fields.path => format!("schema/metric/{}", metric.name()),
                fields.brief => metric.brief(),
                fields.note => metric.note(),
                fields.tag => tags.as_str(),
            ))
            .expect("Failed to add document");
        if let UnivariateMetric::Metric { attributes, .. } = metric {
            attribute::index_schema_attribute(
                attributes.iter(),
                &format!("schema/metric/{}", metric.name()),
                fields,
                index_writer,
            );
        }
    }
}

/// Render a metric details.
pub fn widget<'a>(
    metric: Option<&'a UnivariateMetric>,
    provenance: &'a str,
    theme: &'a ThemeConfig,
) -> Paragraph<'a> {
    match metric {
        Some(metric) => {
            let mut text = vec![Line::from(vec![
                Span::styled("Type      : ", Style::default().fg(theme.label)),
                Span::raw("Metric (schema)"),
            ])];

            if let UnivariateMetric::Metric {
                name,
                brief,
                note,
                attributes,
                instrument,
                unit,
                tags,
            } = metric
            {
                text.push(Line::from(vec![
                    Span::styled("Name      : ", Style::default().fg(theme.label)),
                    Span::raw(name),
                ]));
                text.push(Line::from(vec![
                    Span::styled("Brief     : ", Style::default().fg(theme.label)),
                    Span::raw(brief),
                ]));
                text.push(Line::from(vec![
                    Span::styled("Note      : ", Style::default().fg(theme.label)),
                    Span::raw(note),
                ]));

                text.push(Line::from(vec![
                    Span::styled("Instrument: ", Style::default().fg(theme.label)),
                    Span::raw(format!("{:?}", instrument)),
                ]));

                if let Some(unit) = unit {
                    text.push(Line::from(vec![
                        Span::styled("Unit      : ", Style::default().fg(theme.label)),
                        Span::raw(unit),
                    ]));
                }

                attributes::append_lines(attributes.as_slice(), &mut text, theme);

                tags::append_lines(tags.as_ref(), &mut text, theme);

                // Provenance
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    "Provenance: ",
                    Style::default().fg(theme.label),
                )));
                text.push(Line::from(provenance));
            }
            Paragraph::new(text).style(Style::default().fg(theme.value))
        }
        None => Paragraph::new(vec![Line::default()]),
    }
}
