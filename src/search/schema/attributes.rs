// SPDX-License-Identifier: Apache-2.0

//! List of attributes rendering.

use crate::search::theme::ThemeConfig;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use weaver_schema::attribute::Attribute;
use weaver_semconv::attribute::{BasicRequirementLevelSpec, RequirementLevel};

/// Append attributes to the text.
pub fn append_lines(attributes: &[Attribute], text: &mut Vec<Line>, theme: &ThemeConfig) {
    if !attributes.is_empty() {
        text.push(Line::from(Span::styled(
            "Attributes: ",
            Style::default().fg(theme.label),
        )));
        for attr in attributes.iter() {
            if let Attribute::Id {
                id,
                r#type,
                requirement_level,
                tags,
                value,
                ..
            } = attr
            {
                let mut properties = vec![format!("type={}", r#type)];
                if let RequirementLevel::Basic(BasicRequirementLevelSpec::Required) =
                    requirement_level
                {
                    properties.push("required".to_string());
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
                if let Some(value) = value {
                    properties.push(format!("value={}", value));
                }
                let properties = if properties.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", properties.join(", "))
                };
                text.push(Line::from(Span::raw(format!("- {}{}", id, properties))));
            }
        }
    }
}
