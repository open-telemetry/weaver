// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define spans going forward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    group::{GroupSpec, GroupType, SpanKindSpec},
    v2::{attribute::AttributeRef, CommonFields},
};

/// A group defines an attribute group, an entity, or a signal.
/// Supported group types are: `attribute_group`, `span`, `event`, `metric`, `entity`, `scope`.
/// Mandatory fields are: `id` and `brief`.
///
/// Note: The `resource` type is no longer used and is an alias for `entity`.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpanGroup {
    /// The type of the Span. This denotes the identity
    /// of the "shape" of this span, and must be unique.
    pub r#type: String,
    /// Specifies the kind of the span.
    /// Note: only valid if type is span
    pub kind: SpanKindSpec,
    /// The name patern for the span.
    pub name: String,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeRef>,
    /// Which resources this span should be associated with.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<String>,
    /// List of strings that specify the ids of event semantic conventions
    /// associated with this span semantic convention.
    /// Note: only valid if type is span
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<String>,
    /// Common fields (like brief, note, attributes).
    #[serde(flatten)]
    pub common: CommonFields,
}

impl SpanGroup {
    /// Converts a v2 span gorup into a v1 GroupSpec.
    pub fn into_v1_group(self) -> GroupSpec {
        GroupSpec {
            id: format!("span.{}", &self.r#type),
            r#type: GroupType::Span,
            brief: self.common.brief,
            note: self.common.note,
            prefix: Default::default(),
            extends: None,
            stability: Some(self.common.stability),
            deprecated: self.common.deprecated,
            attributes: self
                .attributes
                .into_iter()
                .map(|a| a.into_v1_attribute())
                .collect(),
            span_kind: Some(self.kind),
            events: self.events,
            metric_name: None,
            instrument: None,
            unit: None,
            name: Some(self.name),
            display_name: None,
            body: None,
            annotations: self.common.annotations,
            entity_associations: self.entity_associations,
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_translate(v2: &str, v1: &str) {
        let span = serde_yaml::from_str::<SpanGroup>(v2).expect("Failed to parse YAML string");
        let expected =
            serde_yaml::from_str::<GroupSpec>(v1).expect("Failed to parse expected YAML");
        assert_eq!(expected, span.into_v1_group());
    }

    #[test]
    fn test_value_spec_display() {
        parse_and_translate(
            // V2 - Span
            r#"type: my_span
name: "{some} {name}"
stability: stable
kind: client
brief: Test span
"#,
            // V1 - Group
            r#"id: span.my_span
type: span
brief: Test span
name: "{some} {name}"
span_kind: client
stability: stable
"#,
        );
    }
}