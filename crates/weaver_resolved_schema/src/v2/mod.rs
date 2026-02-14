//! Version 2 of semantic convention schema.

use std::collections::{HashMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    deprecated::Deprecated,
    group::GroupType,
    manifest::SchemaUrl,
    v2::{
        attribute_group::AttributeGroupVisibilitySpec, signal_id::SignalId, span::SpanName,
        CommonFields,
    },
};
use weaver_version::v2::{RegistryChanges, SchemaChanges, SchemaItemChange};

use crate::{
    v2::{
        attribute::Attribute,
        attribute_group::AttributeGroup,
        catalog::{AttributeCatalog, Catalog},
        entity::Entity,
        metric::Metric,
        refinements::Refinements,
        registry::Registry,
        span::{Span, SpanRefinement},
        stats::Stats,
    },
    V2_RESOLVED_FILE_FORMAT,
};

pub mod attribute;
pub mod attribute_group;
pub mod catalog;
pub mod entity;
pub mod event;
pub mod metric;
pub mod refinements;
pub mod registry;
pub mod span;
pub mod stats;

/// A Resolved Telemetry Schema.
/// A Resolved Telemetry Schema is self-contained and doesn't contain any
/// external references to other schemas or semantic conventions.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedTelemetrySchema {
    /// Version of the file structure.
    pub file_format: String,
    /// Schema URL that this file is published at.
    pub schema_url: SchemaUrl,
    /// Catalog of attributes. Note: this will include duplicates for the same key.
    pub attribute_catalog: Vec<Attribute>,
    /// The registry that this schema belongs to.
    pub registry: Registry,
    /// Refinements for the registry
    pub refinements: Refinements,
}

impl ResolvedTelemetrySchema {
    /// Statistics about this schema.
    pub fn stats(&self) -> Stats {
        Stats {
            registry: self.registry.stats(&self.attribute_catalog),
            refinements: self.refinements.stats(),
        }
    }

    /// Generate a diff between the current schema (must be the most recent one)
    /// and a baseline schema.
    #[must_use]
    pub fn diff(&self, baseline_schema: &ResolvedTelemetrySchema) -> SchemaChanges {
        // TODO - get manifests
        SchemaChanges {
            registry: self.registry_diff(baseline_schema),
        }
    }

    #[must_use]
    fn registry_diff(&self, baseline_schema: &ResolvedTelemetrySchema) -> RegistryChanges {
        RegistryChanges {
            attribute_changes: self.registry_attribute_diff(baseline_schema),
            attribute_group_changes: diff_signals(
                &self.registry.attribute_groups,
                &baseline_schema.registry.attribute_groups,
            ),
            entity_changes: diff_signals(
                &self.registry.entities,
                &baseline_schema.registry.entities,
            ),
            event_changes: diff_signals(&self.registry.events, &baseline_schema.registry.events),
            metric_changes: diff_signals(&self.registry.metrics, &baseline_schema.registry.metrics),
            span_changes: diff_signals(&self.registry.spans, &baseline_schema.registry.spans),
        }
    }

    #[must_use]
    fn registry_attribute_diff(
        &self,
        baseline_schema: &ResolvedTelemetrySchema,
    ) -> Vec<SchemaItemChange> {
        let latest_attributes = self.registry_attribute_map();
        let baseline_attributes = baseline_schema.registry_attribute_map();
        diff_signals_by_hash(&latest_attributes, &baseline_attributes)
    }

    /// Get the registry attributes of the resolved telemetry schema in a fast lookup map.
    fn registry_attribute_map(&self) -> HashMap<&str, &Attribute> {
        self.registry
            .attributes
            .iter()
            .filter_map(|r| self.attribute_catalog.attribute(r))
            .map(|a| (a.key.as_str(), a))
            .collect()
    }
}

/// Easy conversion from v1 to v2.
impl TryFrom<crate::ResolvedTelemetrySchema> for ResolvedTelemetrySchema {
    type Error = crate::error::Error;
    fn try_from(value: crate::ResolvedTelemetrySchema) -> Result<Self, Self::Error> {
        let (attribute_catalog, registry, refinements) =
            convert_v1_to_v2(value.catalog, value.registry)?;
        let schema_url = SchemaUrl::new(value.schema_url);

        match schema_url.validate() {
            Ok(_) => Ok(ResolvedTelemetrySchema {
                file_format: V2_RESOLVED_FILE_FORMAT.to_owned(),
                schema_url,
                attribute_catalog,
                registry,
                refinements,
            }),
            Err(e) => Err(crate::error::Error::InvalidSchemaUrl {
                url: schema_url.to_string(),
                error: e.clone(),
            }),
        }
    }
}

fn fix_group_id(prefix: &'static str, group_id: &str) -> SignalId {
    if group_id.starts_with(prefix) {
        group_id.trim_start_matches(prefix).to_owned().into()
    } else {
        group_id.to_owned().into()
    }
}

fn fix_span_group_id(group_id: &str) -> SignalId {
    fix_group_id("span.", group_id)
}

/// Converts a V1 registry + catalog to V2.
pub fn convert_v1_to_v2(
    c: crate::catalog::Catalog,
    r: crate::registry::Registry,
) -> Result<(Vec<Attribute>, Registry, Refinements), crate::error::Error> {
    // When pulling attributes, as we collapse things, we need to filter
    // to just unique.
    let attributes: HashSet<Attribute> = c
        .attributes
        .iter()
        .cloned()
        .map(|a| {
            Attribute {
                key: a.name,
                r#type: a.r#type,
                examples: a.examples,
                common: CommonFields {
                    brief: a.brief,
                    note: a.note,
                    // TODO - Check this assumption.
                    stability: a
                        .stability
                        .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                    deprecated: a.deprecated,
                    annotations: a.annotations.unwrap_or_default(),
                },
            }
        })
        .collect();

    let v2_catalog = Catalog::from_attributes(attributes.into_iter().collect());

    // Create a lookup so we can check inheritance.
    let mut group_type_lookup = HashMap::new();
    for g in r.groups.iter() {
        let _ = group_type_lookup.insert(g.id.clone(), g.r#type.clone());
    }
    // Pull signals from the registry and create a new span-focused registry.
    let mut spans = Vec::new();
    let mut span_refinements = Vec::new();
    let mut metrics = Vec::new();
    let mut metric_refinements = Vec::new();
    let mut events = Vec::new();
    let mut event_refinements = Vec::new();
    let mut entities = Vec::new();
    let mut attribute_groups = Vec::new();
    for g in r.groups.iter() {
        match g.r#type {
            GroupType::Span => {
                // Check if we extend another span.
                let is_refinement = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.extends_group.as_ref())
                    .and_then(|parent| group_type_lookup.get(parent))
                    .map(|t| *t == GroupType::Span)
                    .unwrap_or(false);
                // Pull all the attribute references.
                let mut span_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        span_attributes.push(span::SpanAttributeRef {
                            base: a,
                            requirement_level: attr.requirement_level.clone(),
                            sampling_relevant: attr.sampling_relevant,
                        });
                    } else {
                        // TODO logic error!
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                if !is_refinement {
                    let span = Span {
                        r#type: fix_span_group_id(&g.id),
                        kind: g
                            .span_kind
                            .clone()
                            .unwrap_or(weaver_semconv::group::SpanKindSpec::Internal),
                        // TODO - Pass advanced name controls through V1 groups.
                        name: SpanName {
                            note: g.name.clone().unwrap_or_default(),
                        },
                        entity_associations: g.entity_associations.clone(),
                        common: CommonFields {
                            brief: g.brief.clone(),
                            note: g.note.clone(),
                            stability: g
                                .stability
                                .clone()
                                .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                            deprecated: g.deprecated.clone(),
                            annotations: g.annotations.clone().unwrap_or_default(),
                        },
                        attributes: span_attributes,
                    };
                    spans.push(span.clone());
                    span_refinements.push(SpanRefinement {
                        id: span.r#type.clone(),
                        span,
                    });
                } else {
                    // unwrap should be safe because we verified this is a refinement earlier.
                    let span_type = g
                        .lineage
                        .as_ref()
                        .and_then(|l| l.extends_group.as_ref())
                        .map(|id| fix_span_group_id(id))
                        .expect("Refinement extraction issue - this is a logic bug");
                    span_refinements.push(SpanRefinement {
                        id: fix_span_group_id(&g.id),
                        span: Span {
                            r#type: span_type,
                            kind: g
                                .span_kind
                                .clone()
                                .unwrap_or(weaver_semconv::group::SpanKindSpec::Internal),
                            // TODO - Pass advanced name controls through V1 groups.
                            name: SpanName {
                                note: g.name.clone().unwrap_or_default(),
                            },
                            entity_associations: g.entity_associations.clone(),
                            common: CommonFields {
                                brief: g.brief.clone(),
                                note: g.note.clone(),
                                stability: g
                                    .stability
                                    .clone()
                                    .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                                deprecated: g.deprecated.clone(),
                                annotations: g.annotations.clone().unwrap_or_default(),
                            },
                            attributes: span_attributes,
                        },
                    });
                }
            }
            GroupType::Event => {
                let is_refinement = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.extends_group.as_ref())
                    .and_then(|parent| group_type_lookup.get(parent))
                    .map(|t| *t == GroupType::Event)
                    .unwrap_or(false);
                let mut event_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        event_attributes.push(event::EventAttributeRef {
                            base: a,
                            requirement_level: attr.requirement_level.clone(),
                        });
                    } else {
                        // TODO logic error!
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                // We cannot convert older repositories before event name was required.
                if let Some(name) = g.name.clone() {
                    let event = event::Event {
                        name: name.into(),
                        attributes: event_attributes,
                        entity_associations: g.entity_associations.clone(),
                        common: CommonFields {
                            brief: g.brief.clone(),
                            note: g.note.clone(),
                            stability: g
                                .stability
                                .clone()
                                .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                            deprecated: g.deprecated.clone(),
                            annotations: g.annotations.clone().unwrap_or_default(),
                        },
                    };
                    if !is_refinement {
                        events.push(event.clone());
                        event_refinements.push(event::EventRefinement {
                            id: event.name.clone(),
                            event,
                        });
                    } else {
                        event_refinements.push(event::EventRefinement {
                            id: fix_group_id("event.", &g.id),
                            event,
                        });
                    }
                } else {
                    // We have no event name
                    return Err(crate::error::Error::EventNameNotFound {
                        group_id: g.id.clone(),
                    });
                }
            }
            GroupType::Metric => {
                // Check if we extend another metric.
                let is_refinement = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.extends_group.as_ref())
                    .and_then(|parent| group_type_lookup.get(parent))
                    .map(|t| *t == GroupType::Metric)
                    .unwrap_or(false);
                let mut metric_attributes = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        metric_attributes.push(metric::MetricAttributeRef {
                            base: a,
                            requirement_level: attr.requirement_level.clone(),
                        });
                    } else {
                        // TODO logic error!
                        log::info!("Logic failure - unable to convert attribute {attr:?}");
                    }
                }
                // TODO - deal with unwrap errors.
                let metric = Metric {
                    name: g
                        .metric_name
                        .clone()
                        .expect("metric_name must exist on metrics prior to translation to v2")
                        .into(),
                    instrument: g
                        .instrument
                        .clone()
                        .expect("instrument must exist on metrics prior to translation to v2"),
                    unit: g
                        .unit
                        .clone()
                        .expect("unit must exist on metrics prior to translation to v2"),
                    attributes: metric_attributes,
                    entity_associations: g.entity_associations.clone(),
                    common: CommonFields {
                        brief: g.brief.clone(),
                        note: g.note.clone(),
                        stability: g
                            .stability
                            .clone()
                            .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                        deprecated: g.deprecated.clone(),
                        annotations: g.annotations.clone().unwrap_or_default(),
                    },
                };
                if is_refinement {
                    metric_refinements.push(metric::MetricRefinement {
                        id: fix_group_id("metric.", &g.id),
                        metric,
                    });
                } else {
                    metrics.push(metric.clone());
                    metric_refinements.push(metric::MetricRefinement {
                        id: metric.name.clone(),
                        metric,
                    });
                }
            }
            GroupType::Entity => {
                let mut id_attrs = Vec::new();
                let mut desc_attrs = Vec::new();
                for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                    if let Some(a) = v2_catalog.convert_ref(attr) {
                        match attr.role {
                            Some(weaver_semconv::attribute::AttributeRole::Identifying) => {
                                id_attrs.push(entity::EntityAttributeRef {
                                    base: a,
                                    requirement_level: attr.requirement_level.clone(),
                                });
                            }
                            _ => {
                                desc_attrs.push(entity::EntityAttributeRef {
                                    base: a,
                                    requirement_level: attr.requirement_level.clone(),
                                });
                            }
                        }
                    } else {
                        // TODO logic error!
                    }
                }
                entities.push(Entity {
                    r#type: fix_group_id("entity.", &g.id),
                    identity: id_attrs,
                    description: desc_attrs,
                    common: CommonFields {
                        brief: g.brief.clone(),
                        note: g.note.clone(),
                        stability: g
                            .stability
                            .clone()
                            .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                        deprecated: g.deprecated.clone(),
                        annotations: g.annotations.clone().unwrap_or_default(),
                    },
                });
            }
            GroupType::AttributeGroup => {
                if g.visibility
                    .as_ref()
                    .is_some_and(|v| AttributeGroupVisibilitySpec::Public == *v)
                {
                    // Now we need to convert the group.
                    let mut attributes = Vec::new();
                    // TODO - we need to check lineage and remove parent groups.
                    for attr in g.attributes.iter().filter_map(|a| c.attribute(a)) {
                        if let Some(a) = v2_catalog.convert_ref(attr) {
                            attributes.push(a);
                        } else {
                            // TODO logic error!
                        }
                    }
                    attribute_groups.push(AttributeGroup {
                        id: fix_group_id("attribute_group.", &g.id),
                        attributes,
                        common: CommonFields {
                            brief: g.brief.clone(),
                            note: g.note.clone(),
                            stability: g
                                .stability
                                .clone()
                                .unwrap_or(weaver_semconv::stability::Stability::Alpha),
                            deprecated: g.deprecated.clone(),
                            annotations: g.annotations.clone().unwrap_or_default(),
                        },
                    });
                }
            }
            GroupType::MetricGroup | GroupType::Scope | GroupType::Undefined => {
                // Ignored for now, we should probably issue warnings.
            }
        }
    }

    // Now we need to hunt for attribute definitions
    let mut attributes = Vec::new();
    for g in r.groups.iter() {
        for a in g.attributes.iter() {
            if let Some(attr) = c.attribute(a) {
                // Attribute definitions do not have lineage.
                let is_def = g
                    .lineage
                    .as_ref()
                    .and_then(|l| l.attribute(&attr.name))
                    .is_none();
                if is_def {
                    if let Some(v2) = v2_catalog.convert_ref(attr) {
                        attributes.push(v2);
                    } else {
                        // TODO logic error!
                    }
                }
            }
        }
    }

    let v2_registry = Registry {
        attributes,
        spans,
        metrics,
        events,
        entities,
        attribute_groups,
    };
    let v2_refinements = Refinements {
        spans: span_refinements,
        metrics: metric_refinements,
        events: event_refinements,
    };
    Ok((v2_catalog.into(), v2_registry, v2_refinements))
}

/// A trait that defines a signal, used for performing "diff"
pub trait Signal {
    /// The id of the signal.
    fn id(&self) -> &str;
    /// The common fields for the signal.
    fn common(&self) -> &CommonFields;
}

/// Diffs signal registries.
#[must_use]
fn diff_signals<T: Signal>(latest: &[T], baseline: &[T]) -> Vec<SchemaItemChange> {
    let baseline_signals: HashMap<&str, &T> = baseline.iter().map(|s| (s.id(), s)).collect();
    let latest_signals: HashMap<&str, &T> = latest.iter().map(|s| (s.id(), s)).collect();
    diff_signals_by_hash(&latest_signals, &baseline_signals)
}

/// Finds the difference between two signal registries using a hash into the signal id.
fn diff_signals_by_hash<T: Signal>(
    latest: &HashMap<&str, &T>,
    baseline: &HashMap<&str, &T>,
) -> Vec<SchemaItemChange> {
    let mut changes: Vec<SchemaItemChange> = Vec::new();
    for (&signal_id, latest_signal) in latest.iter() {
        let baseline_signal = baseline.get(signal_id);
        if let Some(baseline_signal) = baseline_signal {
            if let Some(deprecated) = latest_signal.common().deprecated.as_ref() {
                // is this a change from the baseline?
                if let Some(baseline_deprecated) = baseline_signal.common().deprecated.as_ref() {
                    if deprecated == baseline_deprecated {
                        continue;
                    }
                }

                match deprecated {
                    Deprecated::Renamed {
                        renamed_to: rename_to,
                        note,
                    } => {
                        changes.push(SchemaItemChange::Renamed {
                            old_name: signal_id.to_owned(),
                            new_name: rename_to.clone(),
                            note: note.clone(),
                        });
                    }
                    Deprecated::Obsoleted { note } => {
                        changes.push(SchemaItemChange::Obsoleted {
                            name: signal_id.to_owned(),
                            note: note.clone(),
                        });
                    }
                    Deprecated::Unspecified { note } | Deprecated::Uncategorized { note } => {
                        changes.push(SchemaItemChange::Uncategorized {
                            name: signal_id.to_owned(),
                            note: note.clone(),
                        });
                    }
                }
            }
        } else {
            changes.push(SchemaItemChange::Added {
                name: signal_id.to_owned(),
            });
        }
    }
    // Any signal in the baseline schema that is not present in the latest schema
    // is considered removed.
    // Note: This should never occur if the registry evolution process is followed.
    // However, detecting this case is useful for identifying a violation of the process.
    for (signal_name, _) in baseline.iter() {
        if !latest.contains_key(signal_name) {
            changes.push(SchemaItemChange::Removed {
                name: (*signal_name).to_owned(),
            });
        }
    }
    changes
}

#[cfg(test)]
mod tests {

    use crate::v2::attribute::{Attribute as AttributeV2, AttributeRef};
    use crate::v2::event::Event;
    use crate::V1_RESOLVED_FILE_FORMAT;
    use crate::{attribute::Attribute, lineage::GroupLineage, registry::Group};
    use weaver_semconv::{provenance::Provenance, stability::Stability};

    use crate::lineage::AttributeLineage;

    use super::*;

    #[test]
    fn test_convert_span_v1_to_v2() {
        let mut v1_catalog = crate::catalog::Catalog::from_attributes(vec![]);
        let test_refs = v1_catalog.add_attributes([
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                ),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Recommended,
                ),
                sampling_relevant: Some(true),
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
        ]);
        let mut refinement_span_lineage = GroupLineage::new(Provenance::new("tmp", "tmp"));
        refinement_span_lineage.extends("span.my-span");
        refinement_span_lineage
            .add_attribute_lineage("test.key".to_owned(), AttributeLineage::new("span.my-span"));
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            groups: vec![
                Group {
                    id: "span.my-span".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[1]],
                    span_kind: Some(weaver_semconv::group::SpanKindSpec::Client),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: Some("my span name".to_owned()),
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                },
                Group {
                    id: "span.custom".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[1]],
                    span_kind: Some(weaver_semconv::group::SpanKindSpec::Client),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: Some("my span name".to_owned()),
                    lineage: Some(refinement_span_lineage),
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                },
            ],
        };

        let (catalog, v2_registry, v2_refinements) =
            convert_v1_to_v2(v1_catalog, v1_registry).expect("Failed to convert v1 to v2");
        // assert only ONE attribute due to sharing.
        assert_eq!(catalog.len(), 1);
        // Assert one attribute shows up, due to lineage.
        assert_eq!(v2_registry.attributes.len(), 1);
        // assert attribute fields not shared show up on ref in span.
        assert_eq!(v2_registry.spans.len(), 1);
        if let Some(span) = v2_registry.spans.first() {
            assert_eq!(span.r#type, "my-span".to_owned().into());
            // Make sure attribute ref carries sampling relevant.
        }
        // Assert we have two refinements (e.g. one real span, one refinement).
        assert_eq!(v2_refinements.spans.len(), 2);
        let span_ref_ids: Vec<String> = v2_refinements
            .spans
            .iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(
            span_ref_ids,
            vec!["my-span".to_owned(), "custom".to_owned()]
        );
    }

    #[test]
    fn test_convert_metric_v1_to_v2() {
        let mut v1_catalog = crate::catalog::Catalog::from_attributes(vec![]);
        let test_refs = v1_catalog.add_attributes([
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
                ),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
            Attribute {
                name: "test.key".to_owned(),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: "".to_owned(),
                examples: None,
                tag: None,
                requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                    weaver_semconv::attribute::BasicRequirementLevelSpec::Recommended,
                ),
                sampling_relevant: Some(true),
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                prefix: false,
                tags: None,
                annotations: None,
                value: None,
                role: None,
            },
        ]);
        let mut refinement_metric_lineage = GroupLineage::new(Provenance::new("tmp", "tmp"));
        refinement_metric_lineage.extends("metric.http");
        refinement_metric_lineage
            .add_attribute_lineage("test.key".to_owned(), AttributeLineage::new("metric.http"));
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            groups: vec![
                Group {
                    id: "metric.http".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[0]],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("http".to_owned()),
                    instrument: Some(weaver_semconv::group::InstrumentSpec::UpDownCounter),
                    unit: Some("s".to_owned()),
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                },
                Group {
                    id: "metric.http.custom".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![test_refs[1]],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("http".to_owned()),
                    instrument: Some(weaver_semconv::group::InstrumentSpec::UpDownCounter),
                    unit: Some("s".to_owned()),
                    name: None,
                    lineage: Some(refinement_metric_lineage),
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: None,
                },
            ],
        };

        let (_, v2_registry, v2_refinements) =
            convert_v1_to_v2(v1_catalog, v1_registry).expect("Failed to convert v1 to v2");
        // assert only ONE attribute due to sharing.
        assert_eq!(v2_registry.attributes.len(), 1);
        // assert attribute fields not shared show up on ref in span.
        assert_eq!(v2_registry.metrics.len(), 1);
        if let Some(metric) = v2_registry.metrics.first() {
            assert_eq!(metric.name, "http".to_owned().into());
            // Make sure attribute ref carries sampling relevant.
        }
        // Assert we have two refinements (e.g. one real span, one refinement).
        assert_eq!(v2_refinements.metrics.len(), 2);
        let metric_ref_ids: Vec<String> = v2_refinements
            .metrics
            .iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(
            metric_ref_ids,
            vec!["http".to_owned(), "http.custom".to_owned()]
        );
    }

    #[test]
    fn test_convert_event_v1_to_v2() {
        let mut v1_catalog = crate::catalog::Catalog::from_attributes(vec![]);
        let test_refs = v1_catalog.add_attributes([Attribute {
            name: "test.key".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            brief: "".to_owned(),
            examples: None,
            tag: None,
            requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
            ),
            sampling_relevant: None,
            note: "".to_owned(),
            stability: Some(Stability::Stable),
            deprecated: None,
            prefix: false,
            tags: None,
            annotations: None,
            value: None,
            role: None,
        }]);
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            groups: vec![Group {
                id: "event.my-event".to_owned(),
                r#type: GroupType::Event,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![test_refs[0]],
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                name: Some("my-event".to_owned()),
                lineage: None,
                display_name: None,
                body: None,
                annotations: None,
                entity_associations: vec![],
                visibility: None,
            }],
        };

        let (_, v2_registry, _) =
            convert_v1_to_v2(v1_catalog, v1_registry).expect("Failed to convert v1 to v2");
        assert_eq!(v2_registry.events.len(), 1);
        if let Some(event) = v2_registry.events.first() {
            assert_eq!(event.name, "my-event".to_owned().into());
        }
    }

    #[test]
    fn test_convert_entity_v1_to_v2() {
        let mut v1_catalog = crate::catalog::Catalog::from_attributes(vec![]);
        let test_refs = v1_catalog.add_attributes([Attribute {
            name: "test.key".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            brief: "".to_owned(),
            examples: None,
            tag: None,
            requirement_level: weaver_semconv::attribute::RequirementLevel::Basic(
                weaver_semconv::attribute::BasicRequirementLevelSpec::Required,
            ),
            sampling_relevant: None,
            note: "".to_owned(),
            stability: Some(Stability::Stable),
            deprecated: None,
            prefix: false,
            tags: None,
            annotations: None,
            value: None,
            role: Some(weaver_semconv::attribute::AttributeRole::Identifying),
        }]);
        let v1_registry = crate::registry::Registry {
            registry_url: "my.schema.url".to_owned(),
            groups: vec![Group {
                id: "entity.my-entity".to_owned(),
                r#type: GroupType::Entity,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: vec![test_refs[0]],
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                name: Some("my-entity".to_owned()),
                lineage: None,
                display_name: None,
                body: None,
                annotations: None,
                entity_associations: vec![],
                visibility: None,
            }],
        };

        let (_, v2_registry, _) =
            convert_v1_to_v2(v1_catalog, v1_registry).expect("Failed to convert v1 to v2");
        assert_eq!(v2_registry.entities.len(), 1);
        if let Some(entity) = v2_registry.entities.first() {
            assert_eq!(entity.r#type, "my-entity".to_owned().into());
            assert_eq!(entity.identity.len(), 1);
        }
    }

    #[test]
    fn test_try_from_v1_to_v2() {
        let v1_schema = crate::ResolvedTelemetrySchema {
            file_format: V1_RESOLVED_FILE_FORMAT.to_owned(),
            schema_url: "http://test/schemas/1.0.0".to_owned(),
            registry_id: "my-registry".to_owned(),
            catalog: crate::catalog::Catalog::from_attributes(vec![]),
            registry: crate::registry::Registry {
                registry_url: "http://test/schemas/1.0.0".to_owned(),
                groups: vec![],
            },
            instrumentation_library: None,
            resource: None,
            dependencies: vec![],
            versions: None,
            registry_manifest: None,
        };

        let v2_schema: Result<ResolvedTelemetrySchema, _> = v1_schema.try_into();
        assert!(v2_schema.is_ok());
        let v2_schema = v2_schema.unwrap();
        assert_eq!(v2_schema.file_format, V2_RESOLVED_FILE_FORMAT);
        assert_eq!(
            v2_schema.schema_url,
            SchemaUrl::new("http://test/schemas/1.0.0".to_owned())
        );
    }

    #[test]
    fn no_diff() {
        let mut baseline = empty_v2_schema();
        baseline.attribute_catalog.push(AttributeV2 {
            key: "test.key".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
        });
        baseline.registry.attributes.push(AttributeRef(0));
        let changes = baseline.diff(&baseline);
        assert!(changes.is_empty());
    }

    #[test]
    fn attribute_diff() {
        let mut baseline = empty_v2_schema();
        baseline.attribute_catalog.push(AttributeV2 {
            key: "test.key".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
        });
        baseline.registry.attributes.push(AttributeRef(0));
        let mut latest = empty_v2_schema();
        latest.attribute_catalog.push(AttributeV2 {
            key: "test.key".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: Some(Deprecated::Renamed {
                    renamed_to: "test.key.new".to_owned(),
                    note: "hated it".to_owned(),
                }),
                annotations: Default::default(),
            },
        });
        latest.attribute_catalog.push(AttributeV2 {
            key: "test.key.new".to_owned(),
            r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
        });
        latest.registry.attributes.push(AttributeRef(0));
        latest.registry.attributes.push(AttributeRef(1));
        let diff = latest.diff(&baseline);
        assert!(!diff.is_empty());
        for attr_change in diff.registry.attribute_changes.iter() {
            match attr_change {
                SchemaItemChange::Renamed {
                    old_name,
                    new_name,
                    note,
                } => {
                    assert_eq!(old_name, "test.key");
                    assert_eq!(new_name, "test.key.new");
                    assert_eq!(note, "hated it");
                }
                SchemaItemChange::Added { name } => {
                    assert_eq!(name, "test.key.new");
                }
                c => panic!("Unexpected change type: {:?}", c),
            }
        }
    }

    #[test]
    fn v2_detect_metric_removed() {
        // Test a user changing a metric name but not using deprecated field.
        let mut baseline = empty_v2_schema();
        baseline.registry.metrics.push(Metric {
            name: "http".to_owned().into(),
            instrument: weaver_semconv::group::InstrumentSpec::UpDownCounter,
            unit: "s".to_owned(),
            attributes: vec![],
            entity_associations: vec![],
            common: CommonFields::default(),
        });
        let mut latest = empty_v2_schema();
        latest.registry.metrics.push(Metric {
            name: "http.renamed".to_owned().into(),
            instrument: weaver_semconv::group::InstrumentSpec::UpDownCounter,
            unit: "s".to_owned(),
            attributes: vec![],
            entity_associations: vec![],
            common: CommonFields::default(),
        });
        let diff = latest.diff(&baseline);
        assert!(!diff.is_empty());
        for change in diff.registry.metric_changes.iter() {
            match change {
                SchemaItemChange::Added { name } => {
                    assert_eq!(name, "http.renamed");
                }
                SchemaItemChange::Removed { name } => {
                    assert_eq!(name, "http");
                }
                c => panic!("Unexpected change type: {:?}", c),
            }
        }
    }

    #[test]
    fn v2_detect_entity_uncategorized_deprecation() {
        // Test a user deprecating an entity with unknown change type.
        let mut baseline = empty_v2_schema();
        baseline.registry.entities.push(Entity {
            common: CommonFields::default(),
            r#type: "test.entity".to_owned().into(),
            identity: vec![],
            description: vec![],
        });
        let mut latest = empty_v2_schema();
        latest.registry.entities.push(Entity {
            common: CommonFields {
                deprecated: Some(Deprecated::Uncategorized {
                    note: "note".to_owned(),
                }),
                ..Default::default()
            },
            r#type: "test.entity".to_owned().into(),
            identity: vec![],
            description: vec![],
        });
        let diff = latest.diff(&baseline);
        assert!(!diff.is_empty());
        for change in diff.registry.metric_changes.iter() {
            match change {
                SchemaItemChange::Uncategorized { name, note } => {
                    assert_eq!(name, "test.entity");
                    assert_eq!(note, "note");
                }
                c => panic!("Unexpected change type: {:?}", c),
            }
        }
    }

    #[test]
    fn v2_detect_event_obsoleted() {
        // Test a user obsoleting an event.
        let mut baseline = empty_v2_schema();
        baseline.registry.events.push(Event {
            common: CommonFields::default(),
            name: "test.event".to_owned().into(),
            attributes: vec![],
            entity_associations: vec![],
        });
        let mut latest = empty_v2_schema();
        latest.registry.events.push(Event {
            name: "test.event".to_owned().into(),
            attributes: vec![],
            entity_associations: vec![],
            common: CommonFields {
                deprecated: Some(Deprecated::Obsoleted {
                    note: "note".to_owned(),
                }),
                ..Default::default()
            },
        });
        let diff = latest.diff(&baseline);
        assert!(!diff.is_empty());
        for change in diff.registry.metric_changes.iter() {
            match change {
                SchemaItemChange::Obsoleted { name, note } => {
                    assert_eq!(name, "test.event");
                    assert_eq!(note, "note");
                }
                c => panic!("Unexpected change type: {:?}", c),
            }
        }
    }

    // create an empty schema for testing.
    fn empty_v2_schema() -> ResolvedTelemetrySchema {
        ResolvedTelemetrySchema {
            file_format: V2_RESOLVED_FILE_FORMAT.to_owned(),
            schema_url: SchemaUrl::new("http://test/schemas/1.0".to_owned()),
            attribute_catalog: vec![],
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                spans: vec![],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            refinements: Refinements {
                spans: vec![],
                metrics: vec![],
                events: vec![],
            },
        }
    }
}
