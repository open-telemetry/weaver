// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define spans going forward.

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    deprecated::Deprecated,
    entity_association::EntityAssociation,
    v2::{
        attribute::AttributeRef, signal_id::SignalId,
        signal_requirement_level::SignalRequirementLevel, stability::Stability, CommonFields,
    },
    YamlValue,
};

/// The span kind.
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord, Copy,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum SpanKindSpec {
    /// An internal span.
    Internal,
    /// A client span.
    Client,
    /// A server span.
    Server,
    /// A producer span.
    Producer,
    /// A consumer span.
    Consumer,
}

impl Display for SpanKindSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SpanKindSpec::Internal => write!(f, "internal"),
            SpanKindSpec::Server => write!(f, "server"),
            SpanKindSpec::Client => write!(f, "client"),
            SpanKindSpec::Producer => write!(f, "producer"),
            SpanKindSpec::Consumer => write!(f, "consumer"),
        }
    }
}

/// A reference to an attribute group for spans.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpanGroupRef {
    /// Reference an existing attribute group by id.
    pub ref_group: String,
}

/// Specification of the span name.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Eq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct SpanName {
    /// Required description of how a span name should be created.
    pub note: String,
}

/// A refinement of an Attribute for a span.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
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

/// A reference to either a span attribute or an attribute group.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum SpanAttributeOrGroupRef {
    /// Reference to a span attribute.
    Attribute(SpanAttributeRef),
    /// Reference to an attribute group.
    Group(SpanGroupRef),
}

/// Helper function to split a vector of SpanAttributeOrGroupRef into separate vectors
/// of SpanAttributeRef and group reference strings
#[must_use]
pub fn split_span_attributes_and_groups(
    attributes: Vec<SpanAttributeOrGroupRef>,
) -> (Vec<SpanAttributeRef>, Vec<String>) {
    let mut attribute_refs = Vec::new();
    let mut groups = Vec::new();

    for item in attributes {
        match item {
            SpanAttributeOrGroupRef::Attribute(attr_ref) => {
                attribute_refs.push(attr_ref);
            }
            SpanAttributeOrGroupRef::Group(group_ref) => {
                groups.push(group_ref.ref_group);
            }
        }
    }

    (attribute_refs, groups)
}

/// Defines a new Span signal.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
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
    ///
    /// The list is an implicit `one_of` (telemetry must satisfy at least one entry); each entry is an
    /// entity reference or a nested `one_of`/`all_of` expression.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<EntityAssociation>,
    /// The requirement level of the span. Defaults to 'recommended' when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<SignalRequirementLevel>,
    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A refinement of an existing span.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpanRefinement {
    /// The ID of the refinement.
    pub id: SignalId,
    /// The name of the span being refined.
    pub r#ref: SignalId,
    /// Overrides the span name specification from the referenced base span.
    /// If set, the entire `name` structure from the refinement replaces the
    /// base span's `name`; otherwise, the base span's `name` is inherited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<SpanName>,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<SpanAttributeOrGroupRef>,
    /// Which resources this span should be associated with.
    ///
    /// The list is an implicit `one_of` (telemetry must satisfy at least one entry); each entry is an
    /// entity reference or a nested `one_of`/`all_of` expression.
    /// Note: This field is currently not propagated during resolution.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<EntityAssociation>,

    /// Refines the brief description of the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// Refines the more elaborate description of the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Refines the stability of the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the signal is deprecated.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// Additional annotations for the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, YamlValue>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::attribute::AttributeRef;

    #[test]
    fn test_span_attribute_ref_rejects_stability_and_deprecated() {
        for (field, name) in [
            ("stability: stable", "stability"),
            ("deprecated:\n  reason: obsoleted", "deprecated"),
        ] {
            let yaml = format!("ref: my.attribute\n{field}\n");
            let err = serde_yaml::from_str::<SpanAttributeRef>(&yaml)
                .expect_err("stability/deprecated must not be allowed on span attribute refs");
            assert!(err.to_string().contains(&format!("unknown field `{name}`")));
        }
    }

    #[test]
    fn test_span_kind_spec_display() {
        assert_eq!(SpanKindSpec::Internal.to_string(), "internal");
        assert_eq!(SpanKindSpec::Server.to_string(), "server");
        assert_eq!(SpanKindSpec::Client.to_string(), "client");
        assert_eq!(SpanKindSpec::Producer.to_string(), "producer");
        assert_eq!(SpanKindSpec::Consumer.to_string(), "consumer");
    }

    #[test]
    fn test_split_span_attributes_and_groups() {
        let items = vec![
            SpanAttributeOrGroupRef::Attribute(SpanAttributeRef {
                base: AttributeRef {
                    r#ref: "http.status".to_owned(),
                    brief: None,
                    examples: None,
                    requirement_level: None,
                    note: None,
                    annotations: Default::default(),
                },
                sampling_relevant: Some(true),
            }),
            SpanAttributeOrGroupRef::Group(SpanGroupRef {
                ref_group: "http.shared".to_owned(),
            }),
        ];

        let (attrs, groups) = split_span_attributes_and_groups(items);
        assert_eq!(attrs.len(), 1);
        assert_eq!(groups.len(), 1);
        assert_eq!(attrs[0].base.r#ref, "http.status");
        assert_eq!(attrs[0].sampling_relevant, Some(true));
        assert_eq!(groups[0], "http.shared");
    }
}
