// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define spans going forward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    attribute::AttributeSpec,
    group::{GroupSpec, GroupType, SpanKindSpec},
    v2::{attribute::AttributeRef, signal_id::SignalId, CommonFields},
};

/// A reference to an attribute group for spans.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpanGroupRef {
    /// Reference an existing attribute group by id.
    pub ref_group: String,
}

/// A reference to either a span attribute or an attribute group.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(untagged)]
pub enum SpanAttributeOrGroupRef {
    /// Reference to a span attribute.
    Attribute(SpanAttributeRef),
    /// Reference to an attribute group.
    Group(SpanGroupRef),
}

/// Helper function to split a vector of SpanAttributeOrGroupRef into separate vectors
/// of AttributeSpec and group reference strings
#[must_use]
pub fn split_span_attributes_and_groups(
    attributes: Vec<SpanAttributeOrGroupRef>,
) -> (Vec<AttributeSpec>, Vec<String>) {
    let mut attribute_refs = Vec::new();
    let mut groups = Vec::new();

    for item in attributes {
        match item {
            SpanAttributeOrGroupRef::Attribute(attr_ref) => {
                attribute_refs.push(attr_ref.into_v1_attribute());
            }
            SpanAttributeOrGroupRef::Group(group_ref) => {
                groups.push(group_ref.ref_group);
            }
        }
    }

    (attribute_refs, groups)
}

/// Defines a new Span signal.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Span {
    /// The type of the Span. This denotes the identity
    /// of the "shape" of this span, and must be unique.
    pub r#type: SignalId,
    /// Specifies the kind of the span.
    /// Note: only valid if type is span
    pub kind: SpanKindSpec,
    /// The name pattern for the span.
    pub name: SpanName,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<SpanAttributeOrGroupRef>,
    /// Which resources this span should be associated with.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<String>,
    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

impl Span {
    /// Converts a v2 span group into a v1 GroupSpec.
    #[must_use]
    pub fn into_v1_group(self) -> GroupSpec {
        let (attribute_refs, include_groups) = split_span_attributes_and_groups(self.attributes);
        GroupSpec {
            id: format!("span.{}", &self.r#type),
            r#type: GroupType::Span,
            brief: self.common.brief,
            note: self.common.note,
            prefix: Default::default(),
            extends: None,
            include_groups,
            stability: Some(self.common.stability),
            deprecated: self.common.deprecated,
            attributes: attribute_refs,
            span_kind: Some(self.kind),
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            name: Some(format!("{}", &self.r#type)),
            display_name: None,
            body: None,
            annotations: if self.common.annotations.is_empty() {
                None
            } else {
                Some(self.common.annotations)
            },
            entity_associations: self.entity_associations,
            visibility: None,
        }
    }
}

/// Specification of the span name.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct SpanName {
    /// Required description of how a span name should be created.
    pub note: String,
}

/// A refinement of an Attribute for a span.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct SpanAttributeRef {
    /// Baseline attribute reference.
    #[serde(flatten)]
    pub base: AttributeRef,
    /// Specifies if the attribute is (especially) relevant for sampling
    /// and thus should be set at span start. It defaults to false.
    /// Note: this field is experimental.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling_relevant: Option<bool>,
}

impl SpanAttributeRef {
    /// Converts a v2 refinement into a v1 AttributeSpec.
    #[must_use]
    pub fn into_v1_attribute(self) -> AttributeSpec {
        AttributeSpec::Ref {
            r#ref: self.base.r#ref,
            brief: self.base.brief,
            examples: self.base.examples,
            tag: None,
            requirement_level: self.base.requirement_level,
            sampling_relevant: self.sampling_relevant,
            note: self.base.note,
            stability: self.base.stability,
            deprecated: self.base.deprecated,
            prefix: false,
            annotations: if self.base.annotations.is_empty() {
                None
            } else {
                Some(self.base.annotations)
            },
            role: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_translate(v2: &str, v1: &str) {
        let span = serde_yaml::from_str::<Span>(v2).expect("Failed to parse YAML string");
        let expected =
            serde_yaml::from_str::<GroupSpec>(v1).expect("Failed to parse expected YAML");
        assert_eq!(expected, span.into_v1_group());
    }

    #[test]
    fn test_value_spec_display() {
        parse_and_translate(
            // V2 - Span
            r#"type: my_span
name:
  note: "{some} {name}"
stability: stable
kind: client
brief: Test span
"#,
            // V1 - Group
            r#"id: span.my_span
type: span
brief: Test span
name: my_span
span_kind: client
stability: stable
"#,
        );
    }
}
