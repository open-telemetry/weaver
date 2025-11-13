// SPDX-License-Identifier: Apache-2.0

//! Registry used during the evaluation of the templates. References to the
//! catalog are resolved to the actual catalog entries to ease the template
//! evaluation.

use crate::error::Error;
use itertools::Itertools;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use weaver_resolved_schema::attribute::Attribute;
use weaver_resolved_schema::catalog::Catalog;
use weaver_resolved_schema::lineage::GroupLineage;
use weaver_resolved_schema::registry::{Group, Registry};
use weaver_semconv::any_value::AnyValueSpec;
use weaver_semconv::deprecated::Deprecated;
use weaver_semconv::group::{GroupType, InstrumentSpec, SpanKindSpec};
use weaver_semconv::stability::Stability;
use weaver_semconv::YamlValue;

/// A resolved semantic convention registry used in the context of the template and policy
/// engines.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedRegistry {
    /// The semantic convention registry url.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub registry_url: String,
    /// A list of semantic convention groups.
    pub groups: Vec<ResolvedGroup>,
}

/// Resolved group specification used in the context of the template engine.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct ResolvedGroup {
    /// The id that uniquely identifies the semantic convention.
    pub id: String,
    /// The type of the group including the specific fields for each type.
    pub r#type: GroupType,
    /// A brief description of the semantic convention.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub brief: String,
    /// A more elaborate description of the semantic convention.
    /// It defaults to an empty string.
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Prefix for the attributes for this semantic convention.
    /// It defaults to an empty string.
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    /// Reference another semantic convention id. It inherits
    /// all attributes defined in the specified semantic
    /// convention.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
    /// Specifies the stability of the semantic convention.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the semantic convention is deprecated. The string
    /// provided as `description` MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,

    /// Specifies the kind of the span.
    /// Note: only valid if type is span
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_kind: Option<SpanKindSpec>,
    /// List of strings that specify the ids of event semantic conventions
    /// associated with this span semantic convention.
    /// Note: only valid if type is span
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<String>,
    /// The metric name as described by the [OpenTelemetry Specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/data-model.md#timeseries-model).
    /// Note: This field is required if type is metric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_name: Option<String>,
    /// The instrument type that should be used to record the metric. Note that
    /// the semantic conventions must be written using the names of the
    /// synchronous instrument types (counter, gauge, updowncounter and
    /// histogram).
    /// For more details: [Metrics semantic conventions - Instrument types](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-types).
    /// Note: This field is required if type is metric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument: Option<InstrumentSpec>,
    /// The unit in which the metric is measured, which should adhere to the
    /// [guidelines](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-units).
    /// Note: This field is required if type is metric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The name of the event. If not specified, the prefix is used.
    /// If prefix is empty (or unspecified), name is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The lineage of the group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage: Option<GroupLineage>,
    /// The readable name for attribute groups used when generating registry tables.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// The body specification used for event semantic conventions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<AnyValueSpec>,
    /// The associated entities of this group.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<String>,
    /// Annotations for the group.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, YamlValue>>,
}

impl ResolvedGroup {
    /// Constructs a Template-friendly groups structure from resolved registry structures.
    pub fn try_from_resolved(group: &Group, catalog: &Catalog) -> Result<Self, Error> {
        let mut errors = Vec::new();
        let id = group.id.clone();
        let group_type = group.r#type.clone();
        let brief = group.brief.clone();
        let note = group.note.clone();
        let prefix = group.prefix.clone();
        let extends = group.extends.clone();
        let stability = group.stability.clone();
        let deprecated = group.deprecated.clone();
        let attributes = group
            .attributes
            .iter()
            .filter_map(|attr_ref| {
                let attr = catalog.attribute(attr_ref).cloned();
                if attr.is_none() {
                    errors.push(Error::AttributeNotFound {
                        group_id: id.clone(),
                        attr_ref: *attr_ref,
                    });
                }
                attr
            })
            .collect();
        let lineage = group.lineage.clone();

        if !errors.is_empty() {
            return Err(Error::CompoundError(errors));
        }
        Ok(ResolvedGroup {
            id,
            r#type: group_type,
            brief,
            note,
            prefix,
            extends,
            stability,
            deprecated,
            attributes,
            span_kind: group.span_kind.clone(),
            events: group.events.clone(),
            metric_name: group.metric_name.clone(),
            instrument: group.instrument.clone(),
            unit: group.unit.clone(),
            name: group.name.clone(),
            lineage,
            display_name: group.display_name.clone(),
            body: group.body.clone(),
            entity_associations: group.entity_associations.clone(),
            annotations: group.annotations.clone(),
        })
    }
}

impl ResolvedRegistry {
    /// Create a new template registry from a resolved registry.
    pub fn try_from_resolved_registry(
        registry: &Registry,
        catalog: &Catalog,
    ) -> Result<Self, Error> {
        let mut errors = Vec::new();

        let groups = registry
            .groups
            .iter()
            .map(|group| {
                let id = group.id.clone();
                let group_type = group.r#type.clone();
                let brief = group.brief.clone();
                let note = group.note.clone();
                let prefix = group.prefix.clone();
                let extends = group.extends.clone();
                let stability = group.stability.clone();
                let deprecated = group.deprecated.clone();
                let attributes = group
                    .attributes
                    .iter()
                    .filter_map(|attr_ref| {
                        let attr = catalog.attribute(attr_ref).cloned();
                        if attr.is_none() {
                            errors.push(Error::AttributeNotFound {
                                group_id: id.clone(),
                                attr_ref: *attr_ref,
                            });
                        }
                        attr
                    })
                    .collect();
                let lineage = group.lineage.clone();

                ResolvedGroup {
                    id,
                    r#type: group_type,
                    brief,
                    note,
                    prefix,
                    extends,
                    stability,
                    deprecated,
                    attributes,
                    span_kind: group.span_kind.clone(),
                    events: group.events.clone(),
                    metric_name: group.metric_name.clone(),
                    instrument: group.instrument.clone(),
                    unit: group.unit.clone(),
                    name: group.name.clone(),
                    lineage,
                    display_name: group.display_name.clone(),
                    body: group.body.clone(),
                    entity_associations: group.entity_associations.clone(),
                    annotations: group.annotations.clone(),
                }
            })
            .sorted_by(|a, b| a.id.cmp(&b.id))
            .collect();

        if !errors.is_empty() {
            return Err(Error::CompoundError(errors));
        }

        Ok(Self {
            registry_url: registry.registry_url.clone(),
            groups,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::ResolvedRegistry;
    use schemars::schema_for;
    use serde_json::to_string_pretty;
    use weaver_resolved_schema::catalog::Catalog;
    use weaver_resolved_schema::registry::{Group, Registry};
    use weaver_semconv::group::GroupType;

    #[test]
    fn test_json_schema_gen() {
        // Ensure the JSON schema can be generated for the TemplateRegistry
        let schema = schema_for!(ResolvedRegistry);

        // Ensure the schema can be serialized to a string
        assert!(to_string_pretty(&schema).is_ok());
    }

    #[test]
    fn test_groups_sorted_deterministically() {
        // Create a registry with groups in non-alphabetical order
        let registry = Registry {
            registry_url: "test".to_owned(),
            groups: vec![
                Group {
                    id: "zebra.group".to_owned(),
                    r#type: GroupType::AttributeGroup,
                    brief: "Zebra group".to_owned(),
                    note: String::new(),
                    prefix: String::new(),
                    extends: None,
                    stability: None,
                    deprecated: None,
                    attributes: vec![],
                    span_kind: None,
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    entity_associations: vec![],
                    annotations: None,
                    visibility: None,
                },
                Group {
                    id: "apple.group".to_owned(),
                    r#type: GroupType::AttributeGroup,
                    brief: "Apple group".to_owned(),
                    note: String::new(),
                    prefix: String::new(),
                    extends: None,
                    stability: None,
                    deprecated: None,
                    attributes: vec![],
                    span_kind: None,
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    entity_associations: vec![],
                    annotations: None,
                    visibility: None,
                },
                Group {
                    id: "middle.group".to_owned(),
                    r#type: GroupType::AttributeGroup,
                    brief: "Middle group".to_owned(),
                    note: String::new(),
                    prefix: String::new(),
                    extends: None,
                    stability: None,
                    deprecated: None,
                    attributes: vec![],
                    span_kind: None,
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    entity_associations: vec![],
                    annotations: None,
                    visibility: None,
                },
            ],
        };

        let catalog = Catalog::from_attributes(vec![]);

        // Convert to resolved registry
        let resolved = ResolvedRegistry::try_from_resolved_registry(&registry, &catalog)
            .expect("Failed to create resolved registry");

        // Verify groups are sorted alphabetically by id
        assert_eq!(resolved.groups.len(), 3);
        assert_eq!(resolved.groups[0].id, "apple.group");
        assert_eq!(resolved.groups[1].id, "middle.group");
        assert_eq!(resolved.groups[2].id, "zebra.group");

        // Verify the sorting is stable across multiple conversions
        let resolved2 = ResolvedRegistry::try_from_resolved_registry(&registry, &catalog)
            .expect("Failed to create resolved registry");
        assert_eq!(resolved.groups, resolved2.groups);
    }
}
