// SPDX-License-Identifier: Apache-2.0

//! Version 2 of semantic convention schema.

use std::collections::{BTreeSet, HashMap};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{deprecated::Deprecated, schema_url::SchemaUrl, v2::CommonFields};
use weaver_version::v2::{RegistryChanges, SchemaChanges, SchemaItemChange};

use crate::v2::{
    attribute::Attribute, catalog::AttributeCatalog, refinements::Refinements, registry::Registry,
    stats::Stats,
};

pub mod attribute;
pub mod attribute_group;
pub mod catalog;
pub mod entity;
pub mod event;
pub mod metric;
pub mod provenance;
pub mod refinements;
pub mod registry;
pub mod span;
pub mod stats;

/// Version string denoting V2 resolved schema.
pub const V2_RESOLVED_FILE_FORMAT: &str = "resolved/2.0";

/// A Resolved Telemetry Schema.
/// A Resolved Telemetry Schema is self-contained and doesn't contain any
/// external references to other schemas or semantic conventions.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct ResolvedTelemetrySchema {
    /// Version of the file structure.
    /// Always `"resolved/2.0"` in this version.
    #[schemars(extend("const" = "resolved/2.0"))]
    pub file_format: String,
    /// Schema URL that this file is published at.
    pub schema_url: SchemaUrl,
    /// Catalog of attributes. Note: this will include duplicates for the same key.
    pub attribute_catalog: Vec<Attribute>,
    /// The registry that this schema belongs to.
    pub registry: Registry,
    /// Refinements for the registry
    pub refinements: Refinements,
    /// Every registry this schema was built from, direct and transitive, sorted by
    /// schema URL. A `DependencyRef` is an index into this set.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub dependencies: BTreeSet<SchemaUrl>,
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
        SchemaChanges {
            head_schema_url: self.schema_url.clone(),
            baseline_schema_url: baseline_schema.schema_url.clone(),
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
                if let Some(baseline_deprecated) = baseline_signal.common().deprecated.as_ref() {
                    if deprecated == baseline_deprecated {
                        continue;
                    }
                }

                match deprecated {
                    Deprecated::Renamed {
                        renamed_to: rename_to,
                        ..
                    } => {
                        changes.push(SchemaItemChange::Renamed {
                            old_name: signal_id.to_owned(),
                            new_name: rename_to.clone(),
                            note: deprecated.note(),
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
    use super::*;
    use crate::v2::attribute::{Attribute as AttributeV2, AttributeRef};
    use crate::v2::entity::Entity;
    use crate::v2::event::Event;
    use crate::v2::metric::Metric;
    use weaver_semconv::v2::stability::Stability;

    #[test]
    fn no_diff() {
        let mut baseline = empty_v2_schema();
        baseline.attribute_catalog.push(AttributeV2 {
            key: "test.key".to_owned(),
            r#type: weaver_semconv::v2::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::v2::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
            provenance: Default::default(),
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
            r#type: weaver_semconv::v2::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::v2::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
            provenance: Default::default(),
        });
        baseline.registry.attributes.push(AttributeRef(0));
        let mut latest = empty_v2_schema();
        latest.attribute_catalog.push(AttributeV2 {
            key: "test.key".to_owned(),
            r#type: weaver_semconv::v2::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::v2::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: Some(Deprecated::Renamed {
                    renamed_to: "test.key.new".to_owned(),
                    note: Some("hated it".to_owned()),
                }),
                annotations: Default::default(),
            },
            provenance: Default::default(),
        });
        latest.attribute_catalog.push(AttributeV2 {
            key: "test.key.new".to_owned(),
            r#type: weaver_semconv::v2::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::v2::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: "test brief".to_owned(),
                note: "test note".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
            provenance: Default::default(),
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
        let mut baseline = empty_v2_schema();
        baseline.registry.metrics.push(Metric {
            name: "http".to_owned().into(),
            instrument: weaver_semconv::v2::metric::InstrumentSpec::UpDownCounter,
            unit: "s".to_owned(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: CommonFields::default(),
            provenance: Default::default(),
        });
        let mut latest = empty_v2_schema();
        latest.registry.metrics.push(Metric {
            name: "http.renamed".to_owned().into(),
            instrument: weaver_semconv::v2::metric::InstrumentSpec::UpDownCounter,
            unit: "s".to_owned(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: CommonFields::default(),
            provenance: Default::default(),
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
        let mut baseline = empty_v2_schema();
        baseline.registry.entities.push(Entity {
            common: CommonFields::default(),
            r#type: "test.entity".to_owned().into(),
            identity: vec![],
            description: vec![],
            requirement_level: None,
            provenance: Default::default(),
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
            requirement_level: None,
            provenance: Default::default(),
        });
        let diff = latest.diff(&baseline);
        assert!(!diff.is_empty());
        for change in diff.registry.entity_changes.iter() {
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
        let mut baseline = empty_v2_schema();
        baseline.registry.events.push(Event {
            common: CommonFields::default(),
            name: "test.event".to_owned().into(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            provenance: Default::default(),
        });
        let mut latest = empty_v2_schema();
        latest.registry.events.push(Event {
            name: "test.event".to_owned().into(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: CommonFields {
                deprecated: Some(Deprecated::Obsoleted {
                    note: "note".to_owned(),
                }),
                ..Default::default()
            },
            provenance: Default::default(),
        });
        let diff = latest.diff(&baseline);
        assert!(!diff.is_empty());
        for change in diff.registry.event_changes.iter() {
            match change {
                SchemaItemChange::Obsoleted { name, note } => {
                    assert_eq!(name, "test.event");
                    assert_eq!(note, "note");
                }
                c => panic!("Unexpected change type: {:?}", c),
            }
        }
    }

    #[test]
    fn v2_detect_span_and_attribute_group_diff() {
        let mut baseline = empty_v2_schema();
        baseline.registry.spans.push(span::Span {
            r#type: "http.client".to_owned().into(),
            kind: weaver_semconv::v2::span::SpanKindSpec::Client,
            name: weaver_semconv::v2::span::SpanName {
                note: "HTTP".to_owned(),
            },
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: CommonFields::default(),
            provenance: Default::default(),
        });
        baseline
            .registry
            .attribute_groups
            .push(attribute_group::AttributeGroup {
                id: "http.client.group".to_owned().into(),
                attributes: vec![],
                common: CommonFields::default(),
                provenance: Default::default(),
            });

        let mut latest = empty_v2_schema();
        latest.registry.spans.push(span::Span {
            r#type: "http.client".to_owned().into(),
            kind: weaver_semconv::v2::span::SpanKindSpec::Client,
            name: weaver_semconv::v2::span::SpanName {
                note: "HTTP".to_owned(),
            },
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: CommonFields {
                deprecated: Some(Deprecated::Renamed {
                    renamed_to: "http.client.v2".to_owned(),
                    note: Some("Use v2".to_owned()),
                }),
                ..Default::default()
            },
            provenance: Default::default(),
        });
        latest
            .registry
            .attribute_groups
            .push(attribute_group::AttributeGroup {
                id: "http.server.group".to_owned().into(),
                attributes: vec![],
                common: CommonFields::default(),
                provenance: Default::default(),
            });

        let diff = latest.diff(&baseline);
        assert!(!diff.is_empty());
        assert_eq!(diff.registry.span_changes.len(), 1);
        match &diff.registry.span_changes[0] {
            SchemaItemChange::Renamed {
                old_name,
                new_name,
                note,
            } => {
                assert_eq!(old_name, "http.client");
                assert_eq!(new_name, "http.client.v2");
                assert_eq!(note, "Use v2");
            }
            c => panic!("Unexpected span change: {:?}", c),
        }

        // Attribute group changes: http.server.group added, http.client.group removed
        assert_eq!(diff.registry.attribute_group_changes.len(), 2);
    }

    #[test]
    fn v2_detect_unchanged_deprecation_skips_diff() {
        let mut baseline = empty_v2_schema();
        let dep = Some(Deprecated::Obsoleted {
            note: "same note".to_owned(),
        });
        baseline.registry.events.push(Event {
            name: "same.event".to_owned().into(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: CommonFields {
                deprecated: dep.clone(),
                ..Default::default()
            },
            provenance: Default::default(),
        });
        let mut latest = empty_v2_schema();
        latest.registry.events.push(Event {
            name: "same.event".to_owned().into(),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: CommonFields {
                deprecated: dep,
                ..Default::default()
            },
            provenance: Default::default(),
        });
        let diff = latest.diff(&baseline);
        assert!(diff.registry.event_changes.is_empty());
    }

    #[test]
    fn v2_resolved_schema_stats() {
        use std::collections::BTreeMap;
        use weaver_semconv::v2::attribute::{
            AttributeType, BasicRequirementLevelSpec, EnumEntriesSpec, PrimitiveOrArrayTypeSpec,
            RequirementLevel, ValueSpec,
        };
        use weaver_semconv::v2::metric::InstrumentSpec;
        use weaver_semconv::v2::span::{SpanKindSpec, SpanName};

        let catalog = vec![
            AttributeV2 {
                key: "http.status_code".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
                examples: None,
                common: CommonFields {
                    brief: "HTTP response status code".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            },
            AttributeV2 {
                key: "http.request.method".to_owned(),
                r#type: AttributeType::Enum {
                    members: vec![
                        EnumEntriesSpec {
                            id: "GET".to_owned(),
                            value: ValueSpec::String("GET".to_owned()),
                            brief: None,
                            note: None,
                            stability: None,
                            deprecated: None,
                            annotations: None,
                        },
                        EnumEntriesSpec {
                            id: "POST".to_owned(),
                            value: ValueSpec::String("POST".to_owned()),
                            brief: None,
                            note: None,
                            stability: None,
                            deprecated: None,
                            annotations: None,
                        },
                        EnumEntriesSpec {
                            id: "DELETE".to_owned(),
                            value: ValueSpec::String("DELETE".to_owned()),
                            brief: None,
                            note: None,
                            stability: None,
                            deprecated: None,
                            annotations: None,
                        },
                    ],
                },
                examples: None,
                common: CommonFields {
                    brief: "HTTP method".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            },
            AttributeV2 {
                key: "network.peer.address".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: CommonFields {
                    brief: "Peer address".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Alpha,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            },
            AttributeV2 {
                key: "k8s.pod.name".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: CommonFields {
                    brief: "Pod name".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Beta,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            },
            AttributeV2 {
                key: "deprecated.old_metric_tag".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean),
                examples: None,
                common: CommonFields {
                    brief: "Old tag".to_owned(),
                    note: "Use new tag".to_owned(),
                    stability: Stability::Development,
                    deprecated: Some(Deprecated::Obsoleted {
                        note: "Replaced".to_owned(),
                    }),
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            },
        ];

        let schema = ResolvedTelemetrySchema {
            file_format: V2_RESOLVED_FILE_FORMAT.to_owned(),
            schema_url: "http://test/schemas/1.0"
                .try_into()
                .expect("Should be valid schema url"),
            attribute_catalog: catalog,
            registry: Registry {
                attributes: vec![
                    AttributeRef(0),
                    AttributeRef(1),
                    AttributeRef(2),
                    AttributeRef(3),
                    AttributeRef(4),
                ],
                spans: vec![
                    span::Span {
                        r#type: "http.server.request".to_owned().into(),
                        kind: SpanKindSpec::Server,
                        name: SpanName {
                            note: "HTTP server span".to_owned(),
                        },
                        attributes: vec![span::SpanAttributeRef {
                            base: AttributeRef(0),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                            sampling_relevant: Some(false),
                        }],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields {
                            brief: "Server request span".to_owned(),
                            note: "".to_owned(),
                            stability: Stability::Stable,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    },
                    span::Span {
                        r#type: "internal.processing".to_owned().into(),
                        kind: SpanKindSpec::Internal,
                        name: SpanName {
                            note: "Background processing".to_owned(),
                        },
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields {
                            brief: "Internal background compute".to_owned(),
                            note: "Worker processing".to_owned(),
                            stability: Stability::Development,
                            deprecated: Some(Deprecated::Obsoleted {
                                note: "Deprecated worker".to_owned(),
                            }),
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    },
                ],
                metrics: vec![
                    Metric {
                        name: "http.server.duration".to_owned().into(),
                        instrument: InstrumentSpec::Histogram,
                        unit: "ms".to_owned(),
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields {
                            brief: "Server request duration".to_owned(),
                            note: "".to_owned(),
                            stability: Stability::Stable,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    },
                    Metric {
                        name: "system.memory.usage".to_owned().into(),
                        instrument: InstrumentSpec::Gauge,
                        unit: "By".to_owned(),
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields {
                            brief: "Memory usage".to_owned(),
                            note: "RAM usage in bytes".to_owned(),
                            stability: Stability::Beta,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    },
                    Metric {
                        name: "rpc.client.requests".to_owned().into(),
                        instrument: InstrumentSpec::Counter,
                        unit: "{requests}".to_owned(),
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields {
                            brief: "RPC requests".to_owned(),
                            note: "".to_owned(),
                            stability: Stability::Development,
                            deprecated: Some(Deprecated::Obsoleted {
                                note: "Use v2".to_owned(),
                            }),
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    },
                ],
                events: vec![
                    Event {
                        name: "process.crash".to_owned().into(),
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields {
                            brief: "Process crashed".to_owned(),
                            note: "Crash stacktrace included".to_owned(),
                            stability: Stability::Stable,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    },
                    Event {
                        name: "user.auth_fail".to_owned().into(),
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields {
                            brief: "User auth failed".to_owned(),
                            note: "".to_owned(),
                            stability: Stability::Beta,
                            deprecated: Some(Deprecated::Obsoleted {
                                note: "Replaced with security event".to_owned(),
                            }),
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    },
                ],
                entities: vec![
                    Entity {
                        r#type: "k8s.pod".to_owned().into(),
                        identity: vec![
                            entity::EntityAttributeRef {
                                base: AttributeRef(3),
                                requirement_level: RequirementLevel::Basic(
                                    BasicRequirementLevelSpec::Required,
                                ),
                            },
                            entity::EntityAttributeRef {
                                base: AttributeRef(2),
                                requirement_level: RequirementLevel::Recommended {
                                    text: "Recommended for clustering".to_owned(),
                                },
                            },
                        ],
                        description: vec![],
                        requirement_level: None,
                        common: CommonFields {
                            brief: "Kubernetes pod".to_owned(),
                            note: "".to_owned(),
                            stability: Stability::Stable,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    },
                    Entity {
                        r#type: "host".to_owned().into(),
                        identity: vec![entity::EntityAttributeRef {
                            base: AttributeRef(2),
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                        }],
                        description: vec![],
                        requirement_level: None,
                        common: CommonFields {
                            brief: "Host machine".to_owned(),
                            note: "Physical or VM host".to_owned(),
                            stability: Stability::Alpha,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    },
                ],
                attribute_groups: vec![
                    attribute_group::AttributeGroup {
                        id: "http.client.headers".to_owned().into(),
                        attributes: vec![],
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                    attribute_group::AttributeGroup {
                        id: "network.connection".to_owned().into(),
                        attributes: vec![],
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                ],
            },
            refinements: Refinements {
                spans: vec![span::SpanRefinement {
                    id: "http.server.request".to_owned().into(),
                    span: span::Span {
                        r#type: "http.server.request".to_owned().into(),
                        kind: SpanKindSpec::Server,
                        name: SpanName {
                            note: "HTTP server span".to_owned(),
                        },
                        attributes: vec![],
                        entity_associations: vec![],
                        requirement_level: None,
                        common: CommonFields::default(),
                        provenance: Default::default(),
                    },
                }],
                metrics: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: BTreeSet::new(),
        };

        let stats = schema.stats();

        // Validate attribute statistics
        assert_eq!(stats.registry.attributes.attribute_count, 5);
        assert_eq!(
            stats
                .registry
                .attributes
                .attribute_type_breakdown
                .get("int"),
            Some(&1)
        );
        assert_eq!(
            stats
                .registry
                .attributes
                .attribute_type_breakdown
                .get("string"),
            Some(&2)
        );
        assert_eq!(
            stats
                .registry
                .attributes
                .attribute_type_breakdown
                .get("enum(card:003)"),
            Some(&1)
        );
        assert_eq!(
            stats
                .registry
                .attributes
                .attribute_type_breakdown
                .get("boolean"),
            Some(&1)
        );
        assert_eq!(
            stats
                .registry
                .attributes
                .stability_breakdown
                .get(&Stability::Stable),
            Some(&2)
        );
        assert_eq!(
            stats
                .registry
                .attributes
                .stability_breakdown
                .get(&Stability::Alpha),
            Some(&1)
        );
        assert_eq!(
            stats
                .registry
                .attributes
                .stability_breakdown
                .get(&Stability::Beta),
            Some(&1)
        );
        assert_eq!(
            stats
                .registry
                .attributes
                .stability_breakdown
                .get(&Stability::Development),
            Some(&1)
        );
        assert_eq!(stats.registry.attributes.deprecated_count, 1);

        // Validate span statistics
        assert_eq!(stats.registry.spans.common.count, 2);
        assert_eq!(
            stats
                .registry
                .spans
                .span_kind_breakdown
                .get(&SpanKindSpec::Server),
            Some(&1)
        );
        assert_eq!(
            stats
                .registry
                .spans
                .span_kind_breakdown
                .get(&SpanKindSpec::Internal),
            Some(&1)
        );
        assert_eq!(
            stats
                .registry
                .spans
                .common
                .stability_breakdown
                .get(&Stability::Stable),
            Some(&1)
        );
        assert_eq!(
            stats
                .registry
                .spans
                .common
                .stability_breakdown
                .get(&Stability::Development),
            Some(&1)
        );
        assert_eq!(stats.registry.spans.common.total_with_note, 1);
        assert_eq!(stats.registry.spans.common.deprecated_count, 1);

        // Validate metric statistics
        assert_eq!(stats.registry.metrics.common.count, 3);
        assert_eq!(
            stats
                .registry
                .metrics
                .instrument_breakdown
                .get(&InstrumentSpec::Histogram),
            Some(&1)
        );
        assert_eq!(
            stats
                .registry
                .metrics
                .instrument_breakdown
                .get(&InstrumentSpec::Gauge),
            Some(&1)
        );
        assert_eq!(
            stats
                .registry
                .metrics
                .instrument_breakdown
                .get(&InstrumentSpec::Counter),
            Some(&1)
        );
        assert_eq!(stats.registry.metrics.unit_breakdown.get("ms"), Some(&1));
        assert_eq!(stats.registry.metrics.unit_breakdown.get("By"), Some(&1));
        assert_eq!(
            stats.registry.metrics.unit_breakdown.get("{requests}"),
            Some(&1)
        );
        assert_eq!(stats.registry.metrics.metric_names.len(), 3);
        assert!(stats
            .registry
            .metrics
            .metric_names
            .contains("http.server.duration"));
        assert!(stats
            .registry
            .metrics
            .metric_names
            .contains("system.memory.usage"));
        assert!(stats
            .registry
            .metrics
            .metric_names
            .contains("rpc.client.requests"));
        assert_eq!(stats.registry.metrics.common.total_with_note, 1);
        assert_eq!(stats.registry.metrics.common.deprecated_count, 1);

        // Validate event statistics
        assert_eq!(stats.registry.events.common.count, 2);
        assert!(stats.registry.events.event_names.contains("process.crash"));
        assert!(stats.registry.events.event_names.contains("user.auth_fail"));
        assert_eq!(stats.registry.events.common.total_with_note, 1);
        assert_eq!(stats.registry.events.common.deprecated_count, 1);

        // Validate entity statistics
        assert_eq!(stats.registry.entities.common.count, 2);
        assert!(stats.registry.entities.entity_types.contains("k8s.pod"));
        assert!(stats.registry.entities.entity_types.contains("host"));
        assert_eq!(
            stats
                .registry
                .entities
                .entity_identity_length_distribution
                .get(&2),
            Some(&1)
        );
        assert_eq!(
            stats
                .registry
                .entities
                .entity_identity_length_distribution
                .get(&1),
            Some(&1)
        );
        assert_eq!(stats.registry.entities.common.total_with_note, 1);

        // Validate attribute group statistics
        assert_eq!(stats.registry.attribute_groups.common.count, 2);
    }

    fn empty_v2_schema() -> ResolvedTelemetrySchema {
        ResolvedTelemetrySchema {
            file_format: V2_RESOLVED_FILE_FORMAT.to_owned(),
            schema_url: "http://test/schemas/1.0"
                .try_into()
                .expect("Should be valid schema url"),
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
                entities: vec![],
            },
            dependencies: BTreeSet::new(),
        }
    }
}
