// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define spans going forward.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    attribute::{AttributeSpec, RequirementLevel},
    deprecated::Deprecated,
    entity_association::EntityAssociation,
    group::{GroupSpec, GroupType, SpanKindSpec},
    signal_requirement_level::SignalRequirementLevel,
    stability::Stability,
    v2::{attribute::AttributeRef, signal_id::SignalId, CommonFields},
    YamlValue,
};

/// A reference to an attribute group for spans.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpanGroupRef {
    /// Reference an existing attribute group by id.
    pub ref_group: String,
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

/// Declares a link from this span to another span.
///
/// Span links model relations that do not fit the parent/child tree,
/// for example a batch consumer span that links to the creation
/// context of each message it processes.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct SpanLink {
    /// The span type this link points to.
    pub r#ref: SignalId,
    /// The requirement level of the link. Uses the attribute requirement
    /// levels ("required", "conditionally_required", "recommended",
    /// "opt_in") because a link, unlike a signal, can be required.
    /// Defaults to 'recommended' when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<RequirementLevel>,
    /// Refines the brief description of the link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// Refines the more elaborate description of the link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// List of attributes expected on the link itself.
    /// Attribute-group references are not supported on links; each entry
    /// is a single attribute reference.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<SpanAttributeRef>,
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
    ///
    /// The list is an implicit `one_of` (telemetry must satisfy at least one entry); each entry is an
    /// entity reference or a nested `one_of`/`all_of` expression.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<EntityAssociation>,
    /// Declares links from this span to other spans.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<SpanLink>,
    /// The requirement level of the span. Defaults to 'recommended' when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<SignalRequirementLevel>,
    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A refinement of an existing span.
///
/// A refinement inherits the base span's links during resolution.
/// A refinement cannot declare, replace, or extend links yet.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
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
            is_v2: true,
            span_name: Some(self.name),
            span_links: self.links,
            requirement_level: self.requirement_level,
        }
    }
}

impl SpanRefinement {
    /// Converts a v2 span refinement into a v1 GroupSpec.
    #[must_use]
    pub fn into_v1_group(self) -> GroupSpec {
        let (attribute_refs, include_groups) = split_span_attributes_and_groups(self.attributes);
        GroupSpec {
            id: self.id.to_string(),
            r#type: GroupType::Span,
            brief: self.brief.unwrap_or_default(),
            note: self.note.unwrap_or_default(),
            prefix: Default::default(),
            extends: Some(format!("span.{}", &self.r#ref)),
            include_groups,
            stability: self.stability,
            deprecated: self.deprecated,
            attributes: attribute_refs,
            span_kind: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            name: Some(format!("{}", &self.id)),
            display_name: None,
            body: None,
            annotations: if self.annotations.is_empty() {
                None
            } else {
                Some(self.annotations)
            },
            entity_associations: self.entity_associations,
            visibility: None,
            is_v2: true,
            span_name: self.name,
            span_links: Vec::new(),
            requirement_level: None,
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
            stability: None,
            deprecated: None,
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
is_v2: true
span_name:
  note: "{some} {name}"
"#,
        );
    }

    fn parse_and_translate_refinement(v2: &str, v1: &str) {
        let span = serde_yaml::from_str::<SpanRefinement>(v2).expect("Failed to parse YAML string");
        let expected =
            serde_yaml::from_str::<GroupSpec>(v1).expect("Failed to parse expected YAML");
        assert_eq!(expected, span.into_v1_group());
    }

    #[test]
    fn test_span_requirement_level_translation() {
        parse_and_translate(
            // V2 - Span
            r#"type: my_span
name:
  note: "{some} {name}"
stability: stable
kind: client
brief: Test span
requirement_level: opt_in
"#,
            // V1 - Group
            r#"id: span.my_span
type: span
brief: Test span
name: my_span
span_kind: client
stability: stable
is_v2: true
span_name:
  note: "{some} {name}"
requirement_level: opt_in
"#,
        );
    }

    #[test]
    fn test_span_refinement_translation() {
        parse_and_translate_refinement(
            // V2 - SpanRefinement
            r#"id: span.refinement.my_span
ref: my_span
brief: Test span refinement
stability: stable
"#,
            // V1 - Group
            r#"id: span.refinement.my_span
type: span
brief: Test span refinement
name: span.refinement.my_span
extends: span.my_span
stability: stable
is_v2: true
"#,
        );
    }

    #[test]
    fn test_span_refinement_with_name_override() {
        parse_and_translate_refinement(
            // V2 - SpanRefinement with name override
            r#"id: span.refinement.my_span
ref: my_span
name:
  note: "{gen_ai.operation.name} {gen_ai.request.model}"
brief: Test span refinement with custom name
stability: stable
"#,
            // V1 - Group. The group `name` stays the refinement id, but the
            // overriding span name format is carried in `span_name`.
            r#"id: span.refinement.my_span
type: span
brief: Test span refinement with custom name
name: span.refinement.my_span
extends: span.my_span
stability: stable
is_v2: true
span_name:
  note: "{gen_ai.operation.name} {gen_ai.request.model}"
"#,
        );
    }

    #[test]
    fn test_span_links_deserialization() {
        // A span with two links: one minimal, one full.
        let span: Span = serde_yaml::from_str(
            r#"type: messaging.consumer.process
name:
  note: "process {messaging.destination.name}"
stability: stable
kind: consumer
brief: Processes a batch of messages.
links:
  - ref: messaging.producer.publish
  - ref: messaging.producer.publish
    requirement_level: required
    brief: One link per message in the batch.
    attributes:
      - ref: messaging.message.id
"#,
        )
        .expect("Failed to parse span with links");

        assert_eq!(span.links.len(), 2);

        // The minimal link: only `ref`; everything else defaults.
        let minimal = &span.links[0];
        assert_eq!(minimal.r#ref.to_string(), "messaging.producer.publish");
        assert!(minimal.requirement_level.is_none());
        assert!(minimal.brief.is_none());
        assert!(minimal.attributes.is_empty());

        // The full link carries a level, a brief, and one attribute ref.
        let full = &span.links[1];
        assert_eq!(
            full.requirement_level,
            Some(RequirementLevel::Basic(
                crate::attribute::BasicRequirementLevelSpec::Required
            ))
        );
        assert_eq!(
            full.brief.as_deref(),
            Some("One link per message in the batch.")
        );
        assert_eq!(full.attributes.len(), 1);

        // A span without a `links` key parses to an empty list.
        let without: Span = serde_yaml::from_str(
            r#"type: my_span
name:
  note: "{some} {name}"
stability: stable
kind: client
brief: Test span
"#,
        )
        .expect("Failed to parse span without links");
        assert!(without.links.is_empty());
    }

    #[test]
    fn test_span_link_rejects_attribute_group_ref() {
        // Attribute-group references are not supported on links; the
        // parser must reject them instead of resolution failing later.
        // Two shapes exist: a bare group entry fails on the missing
        // `ref`, and a group key next to a valid `ref` fails as an
        // unknown field.
        let span_yaml = |attribute_entry: &str| {
            format!(
                r#"type: my_span
name:
  note: "{{some}} {{name}}"
stability: stable
kind: client
brief: Test span
links:
  - ref: other_span
    attributes:
      - {attribute_entry}
"#
            )
        };
        let err = serde_yaml::from_str::<Span>(&span_yaml("ref_group: some.group"))
            .expect_err("a bare ref_group entry must not parse");
        assert!(
            err.to_string().contains("missing field `ref`"),
            "unexpected parse error: {err}"
        );
        let err = serde_yaml::from_str::<Span>(&span_yaml(
            "ref: real.attr\n        ref_group: some.group",
        ))
        .expect_err("a ref_group key next to a ref must not parse");
        assert!(
            err.to_string().contains("unknown field `ref_group`"),
            "unexpected parse error: {err}"
        );
    }

    #[test]
    fn test_span_refinement_name_deserialization() {
        // Verify that a SpanRefinement without name parses correctly
        let without_name: SpanRefinement = serde_yaml::from_str(
            r#"id: my.refinement
ref: base.span
brief: No name override
"#,
        )
        .expect("Failed to parse refinement without name");
        assert!(without_name.name.is_none());

        // Verify that a SpanRefinement with name parses correctly
        let with_name: SpanRefinement = serde_yaml::from_str(
            r#"id: my.refinement
ref: base.span
name:
  note: "{custom} {name_format}"
brief: With name override
"#,
        )
        .expect("Failed to parse refinement with name");
        assert!(with_name.name.is_some());
        assert_eq!(with_name.name.unwrap().note, "{custom} {name_format}");
    }

    #[test]
    fn test_span_attribute_ref_rejects_stability_and_deprecated() {
        for (field, name) in [
            ("stability: stable", "stability"),
            ("deprecated:\n  reason: obsoleted", "deprecated"),
        ] {
            let yaml = format!("ref: my.attribute\nsampling_relevant: true\n{field}\n");
            let err = serde_yaml::from_str::<SpanAttributeRef>(&yaml)
                .expect_err("stability/deprecated must not be allowed on span attribute refs");
            // `serde(flatten)` drops the position and the list of expected fields.
            assert_eq!(err.to_string(), format!("unknown field `{name}`"));
        }
    }
}
