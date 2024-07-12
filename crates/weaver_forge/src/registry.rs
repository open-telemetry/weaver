// SPDX-License-Identifier: Apache-2.0

//! Registry used during the evaluation of the templates. References to the
//! catalog are resolved to the actual catalog entries to ease the template
//! evaluation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use weaver_resolved_schema::attribute::Attribute;
use weaver_resolved_schema::catalog::Catalog;
use weaver_resolved_schema::lineage::GroupLineage;
use weaver_resolved_schema::registry::{Constraint, Group, Registry};
use weaver_semconv::group::{GroupType, InstrumentSpec, SpanKindSpec};
use weaver_semconv::stability::Stability;

use crate::config::{AttributeOrCondition, GroupOrCondition, GroupProcessing};
use crate::error::Error;

/// A resolved semantic convention registry used in the context of the template and policy
/// engines.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedRegistry {
    /// The semantic convention registry url.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub registry_url: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
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
    /// Reference another semantic convention id. It inherits the prefix,
    /// constraints, and all attributes defined in the specified semantic
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
    pub deprecated: Option<String>,
    /// Additional constraints.
    /// Allow to define additional requirements on the semantic convention.
    /// It defaults to an empty list.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<Constraint>,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    pub attributes: Vec<Attribute>,

    /// Specifies the kind of the span.
    /// Note: only valid if type is span (the default)
    pub span_kind: Option<SpanKindSpec>,
    /// List of strings that specify the ids of event semantic conventions
    /// associated with this span semantic convention.
    /// Note: only valid if type is span (the default)
    #[serde(default)]
    pub events: Vec<String>,
    /// The metric name as described by the [OpenTelemetry Specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/data-model.md#timeseries-model).
    /// Note: This field is required if type is metric.
    pub metric_name: Option<String>,
    /// The instrument type that should be used to record the metric. Note that
    /// the semantic conventions must be written using the names of the
    /// synchronous instrument types (counter, gauge, updowncounter and
    /// histogram).
    /// For more details: [Metrics semantic conventions - Instrument types](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-types).
    /// Note: This field is required if type is metric.
    pub instrument: Option<InstrumentSpec>,
    /// The unit in which the metric is measured, which should adhere to the
    /// [guidelines](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-units).
    /// Note: This field is required if type is metric.
    pub unit: Option<String>,
    /// The name of the event. If not specified, the prefix is used.
    /// If prefix is empty (or unspecified), name is required.
    pub name: Option<String>,
    /// The lineage of the group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage: Option<GroupLineage>,
    /// The readable name for attribute groups used when generating registry tables.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
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
        let constraints = group.constraints.clone();
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
            id: id.clone(),
            r#type: group_type,
            brief,
            note,
            prefix,
            extends,
            stability,
            deprecated,
            constraints,
            attributes,
            span_kind: group.span_kind.clone(),
            events: group.events.clone(),
            metric_name: group.metric_name.clone(),
            instrument: group.instrument.clone(),
            unit: group.unit.clone(),
            name: group.name.clone(),
            lineage,
            display_name: group.display_name.clone(),
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
                let constraints = group.constraints.clone();
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
                    id: id.clone(),
                    r#type: group_type,
                    brief,
                    note,
                    prefix,
                    extends,
                    stability,
                    deprecated,
                    constraints,
                    attributes,
                    span_kind: group.span_kind.clone(),
                    events: group.events.clone(),
                    metric_name: group.metric_name.clone(),
                    instrument: group.instrument.clone(),
                    unit: group.unit.clone(),
                    name: group.name.clone(),
                    lineage,
                    display_name: group.display_name.clone(),
                }
            })
            .collect();

        if !errors.is_empty() {
            return Err(Error::CompoundError(errors));
        }

        Ok(Self {
            registry_url: registry.registry_url.clone(),
            groups,
        })
    }

    /// Apply the group processing configuration to the resolved registry.
    pub fn apply_group_processing(&mut self, config: &GroupProcessing) {
        fn any_of_group_conditions(group: &ResolvedGroup, conditions: &GroupOrCondition) -> bool {
            let id_matches = conditions.id_regex.iter().any(|re| re.is_match(&group.id));
            let type_matches = conditions.type_set.iter().any(|type_set| type_set.contains(&group.r#type));
            let stability_matches = conditions.stability_set.iter().any(|stability_set| if let Some(stability) = &group.stability {
                stability_set.contains(&stability)
            } else {
                false
            }
            );
            let without_attributes_matches = conditions.without_attributes.unwrap_or_default() && group.attributes.is_empty();

            id_matches || type_matches || stability_matches || without_attributes_matches
        }
        fn any_of_attribute_conditions(attribute: &Attribute, conditions: &AttributeOrCondition) -> bool {
            let id_matches = conditions.name_regex.iter().any(|re| re.is_match(&attribute.name));
            let stability_matches = conditions.stability_set.iter().any(|stability_set| if let Some(stability) = &attribute.stability {
                stability_set.contains(&stability)
            } else {
                false
            }
            );

            id_matches || stability_matches
        }

        if let Some(conditions) = &config.remove_attributes_with {
            self.groups.iter_mut().for_each(|group| {
                group.attributes.retain(|attr| {
                    !any_of_attribute_conditions(attr, conditions)
                });
            });
        }
        if let Some(conditions) = &config.retain_attributes_with {
            self.groups.iter_mut().for_each(|group| {
                group.attributes.retain(|attr| {
                    any_of_attribute_conditions(attr, conditions)
                });
            });
        }

        if let Some(conditions) = &config.remove_groups_with {
            self.groups.retain(|group| {
                !any_of_group_conditions(group, conditions)
            });
        }
        if let Some(conditions) = &config.retain_groups_with {
            self.groups.retain(|group| {
                any_of_group_conditions(group, conditions)
            });
        }

        if config.sort_groups_by_id.unwrap_or_default() {
            self.groups.sort_by(|a, b| a.id.cmp(&b.id));
        }
    }
}

#[cfg(test)]
mod tests {
    use schemars::schema_for;
    use serde_json::to_string_pretty;

    use crate::ResolvedRegistry;

    #[test]
    fn test_json_schema_gen() {
        // Ensure the JSON schema can be generated for the TemplateRegistry
        let schema = schema_for!(ResolvedRegistry);

        // Ensure the schema can be serialized to a string
        assert!(to_string_pretty(&schema).is_ok());
    }
}
