// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define data going forward.

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use globset::Glob;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use weaver_common::result::WResult;

use crate::{
    deprecated::Deprecated,
    stability::Stability,
    v2::{
        attribute::AttributeDef, attribute::AttributeRef, attribute_group::AttributeGroup,
        entity::Entity, entity::EntityRefinement, event::Event, event::EventRefinement,
        metric::Metric, metric::MetricRefinement, signal_id::SignalId, span::Span,
        span::SpanRefinement,
    },
    Error, YamlValue,
};

pub mod attribute;
pub mod attribute_group;
pub mod entity;
pub mod event;
pub mod metric;
pub mod signal_id;
pub mod span;

/// Common fields we want on all major components of semantic conventions.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Hash, Eq, Default)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct CommonFields {
    /// A brief description of the attribute or signal.
    pub brief: String,
    /// A more elaborate description of the attribute or signal.
    /// It defaults to an empty string.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Specifies the stability of the attribute or signal.
    pub stability: Stability,
    /// Specifies if the semantic convention is deprecated. The string
    /// provided as description MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// Annotations for the attribute or signal.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, YamlValue>,
}

/// A wildcard expression for matching signal groups.
#[derive(Debug, Clone, JsonSchema, PartialEq, Eq)]
pub struct GroupWildcard(#[schemars(with = "String")] pub Glob);

impl Serialize for GroupWildcard {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.glob().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GroupWildcard {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Glob::new(&s)
            .map(GroupWildcard)
            .map_err(serde::de::Error::custom)
    }
}

impl Display for GroupWildcard {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.glob())
    }
}

/// Imports are used to reference groups defined in a dependent registry.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Imports {
    /// A list of metric group metric_name wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Vec<GroupWildcard>>,

    /// A list of event group name wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<GroupWildcard>>,

    /// A list of entity group name wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<Vec<GroupWildcard>>,

    /// A list of span group name wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<GroupWildcard>>,

    /// A list of attribute_group group id wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribute_groups: Option<Vec<GroupWildcard>>,
}

/// A semconv file is a collection of attributes, signals, groups,
/// and imports.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SemConvSpecV2 {
    /// A collection of semantic conventions for attributes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) attributes: Vec<AttributeDef>,
    /// A collection of semantic conventions for Entity signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) entities: Vec<Entity>,
    /// A collection of semantic conventions for Event signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) events: Vec<Event>,
    /// A collection of semantic conventions for Metric signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) metrics: Vec<Metric>,
    /// A collection of semantic conventions for Span signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) spans: Vec<Span>,
    /// A collection of semantic conventions for AttributeGroups.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) attribute_groups: Vec<AttributeGroup>,

    /// A collection of semantic convention refinements for Entity signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) entity_refinements: Vec<EntityRefinement>,
    /// A collection of semantic convention refinements for Event signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) event_refinements: Vec<EventRefinement>,
    /// A collection of semantic convention refinements for Metric signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) metric_refinements: Vec<MetricRefinement>,
    /// A collection of semantic convention refinements for Span signals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) span_refinements: Vec<SpanRefinement>,

    /// A list of imports referencing groups defined in a dependent registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) imports: Option<Imports>,
}

impl SemConvSpecV2 {
    /// Creates a v2 semantic convention spec with the main signal sections.
    #[must_use]
    pub fn new(
        attributes: Vec<AttributeDef>,
        entities: Vec<Entity>,
        events: Vec<Event>,
        metrics: Vec<Metric>,
        spans: Vec<Span>,
    ) -> Self {
        Self {
            attributes,
            entities,
            events,
            metrics,
            spans,
            attribute_groups: vec![],
            entity_refinements: vec![],
            event_refinements: vec![],
            metric_refinements: vec![],
            span_refinements: vec![],
            imports: None,
        }
    }

    /// Accessor for attributes.
    #[must_use]
    pub fn attributes(&self) -> &[AttributeDef] {
        &self.attributes
    }

    /// Accessor for entities.
    #[must_use]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Accessor for events.
    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// Accessor for metrics.
    #[must_use]
    pub fn metrics(&self) -> &[Metric] {
        &self.metrics
    }

    /// Accessor for spans.
    #[must_use]
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// Accessor for attribute groups.
    #[must_use]
    pub fn attribute_groups(&self) -> &[AttributeGroup] {
        &self.attribute_groups
    }

    /// Accessor for entity refinements.
    #[must_use]
    pub fn entity_refinements(&self) -> &[EntityRefinement] {
        &self.entity_refinements
    }

    /// Accessor for event refinements.
    #[must_use]
    pub fn event_refinements(&self) -> &[EventRefinement] {
        &self.event_refinements
    }

    /// Accessor for metric refinements.
    #[must_use]
    pub fn metric_refinements(&self) -> &[MetricRefinement] {
        &self.metric_refinements
    }

    /// Accessor for span refinements.
    #[must_use]
    pub fn span_refinements(&self) -> &[SpanRefinement] {
        &self.span_refinements
    }

    /// Accessor for imports.
    #[must_use]
    pub fn imports(&self) -> Option<&Imports> {
        self.imports.as_ref()
    }

    /// Returns the JSON schema for `SemConvSpecV2` including the injected `file_format` field.
    #[must_use]
    pub fn output_schema() -> schemars::Schema {
        let mut schema =
            serde_json::to_value(schemars::schema_for!(Self)).expect("Failed to serialize schema");
        if let Some(props) = schema.get_mut("properties").and_then(|p| p.as_object_mut()) {
            let _ = props.insert(
                "file_format".to_owned(),
                serde_json::json!({
                    "description": "The file format version.",
                    "type": "string",
                    "const": "definition/2"
                }),
            );
        }
        serde_json::from_value(schema).expect("Failed to deserialize modified schema")
    }

    /// Validates invariants on the model.
    pub fn validate(self, provenance: &str) -> WResult<Self, Error> {
        let mut errors: Vec<Error> = vec![];
        let mut fatal_errors: Vec<Error> = vec![];

        let mut check = |cond: bool, group_id: String| {
            if cond {
                errors.push(Error::MissingRequirementLevelWarning {
                    path_or_url: provenance.to_owned(),
                    group_id,
                });
            }
        };

        for m in &self.metrics {
            check(m.requirement_level.is_none(), format!("metric.{}", m.name));
        }
        for s in &self.spans {
            check(s.requirement_level.is_none(), format!("span.{}", s.r#type));
        }
        for e in &self.events {
            check(e.requirement_level.is_none(), format!("event.{}", e.name));
        }
        for e in &self.entities {
            check(
                e.requirement_level.is_none(),
                format!("entity.{}", e.r#type),
            );
        }

        let check_identity_overlap =
            |identity: &[AttributeRef], description: &[AttributeRef], group_id: &SignalId| {
                let mut overlaps = vec![];
                for attr in description {
                    if identity.iter().any(|i| i.r#ref == attr.r#ref) {
                        overlaps.push(Error::AttributeInIdentityAndDescription {
                            path_or_url: provenance.to_owned(),
                            group_id: group_id.to_string(),
                            attribute_id: attr.r#ref.clone(),
                        });
                    }
                }
                overlaps
            };
        for e in &self.entities {
            fatal_errors.extend(check_identity_overlap(
                &e.identity,
                &e.description,
                &e.r#type,
            ));
            if e.identity.is_empty() {
                fatal_errors.push(Error::EntityMissingIdentity {
                    path_or_url: provenance.to_owned(),
                    group_id: e.r#type.to_string(),
                });
            }
        }
        for r in &self.entity_refinements {
            fatal_errors.extend(check_identity_overlap(&r.identity, &r.description, &r.id));
        }

        if !fatal_errors.is_empty() {
            return WResult::FatalErr(Error::CompoundError(fatal_errors));
        }
        WResult::with_non_fatal_errors(self, errors)
    }

    /// True if this specification holds no definitions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.attributes.is_empty()
            && self.entities.is_empty()
            && self.events.is_empty()
            && self.metrics.is_empty()
            && self.spans.is_empty()
            && self.attribute_groups.is_empty()
            && self.entity_refinements.is_empty()
            && self.event_refinements.is_empty()
            && self.metric_refinements.is_empty()
            && self.span_refinements.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semconv_spec_v2_is_empty() {
        let empty_spec = SemConvSpecV2 {
            attributes: vec![],
            entities: vec![],
            events: vec![],
            metrics: vec![],
            spans: vec![],
            attribute_groups: vec![],
            entity_refinements: vec![],
            event_refinements: vec![],
            metric_refinements: vec![],
            span_refinements: vec![],
            imports: None,
        };
        assert!(empty_spec.is_empty());

        let non_empty_spec = SemConvSpecV2 {
            attributes: vec![AttributeDef {
                key: "test".to_owned(),
                r#type: attribute::AttributeType::PrimitiveOrArray(
                    attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                examples: None,
                common: CommonFields {
                    brief: "test".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: Default::default(),
                },
            }],
            ..empty_spec.clone()
        };
        assert!(!non_empty_spec.is_empty());
    }

    #[test]
    fn test_output_schema_contains_file_format() {
        let schema = SemConvSpecV2::output_schema();
        let value = serde_json::to_value(&schema).expect("Failed to serialize schema");
        let file_format = value
            .get("properties")
            .and_then(|p| p.get("file_format"))
            .expect("Expected 'file_format' in schema properties");
        assert_eq!(
            file_format.get("const").and_then(|v| v.as_str()),
            Some("definition/2")
        );
    }

    #[test]
    fn test_semconv_spec_v2_constructor_and_accessors() {
        let spec = SemConvSpecV2::new(
            vec![AttributeDef {
                key: "test".to_owned(),
                r#type: attribute::AttributeType::PrimitiveOrArray(
                    attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                examples: None,
                common: CommonFields {
                    brief: "test".to_owned(),
                    note: String::new(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: Default::default(),
                },
            }],
            vec![],
            vec![],
            vec![],
            vec![],
        );

        assert_eq!(spec.attributes().len(), 1);
        assert!(spec.entities().is_empty());
        assert!(spec.events().is_empty());
        assert!(spec.metrics().is_empty());
        assert!(spec.spans().is_empty());
        assert!(!spec.is_empty());
    }
}
