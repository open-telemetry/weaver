// SPDX-License-Identifier: Apache-2.0

//! Define the concept of Resolved Telemetry Schema.
//!
//! A Resolved Telemetry Schema is self-contained and doesn't contain any
//! external references to other schemas or semantic conventions.

use crate::attribute::Attribute;
use crate::catalog::Catalog;
use crate::instrumentation_library::InstrumentationLibrary;
use crate::registry::{Group, Registry};
use crate::resource::Resource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use weaver_semconv::deprecated::Deprecated;
use weaver_semconv::group::GroupType;
use weaver_semconv::manifest::RegistryManifest;
use weaver_version::schema_changes::{SchemaChanges, SchemaItemChange, SchemaItemType};
use weaver_version::Versions;

pub mod attribute;
pub mod catalog;
pub mod error;
pub mod instrumentation_library;
pub mod lineage;
pub mod metric;
pub mod registry;
pub mod resource;
pub mod signal;
pub mod tags;
pub mod value;

/// The registry ID for the OpenTelemetry semantic conventions.
/// This ID is reserved and should not be used by any other registry.
pub const OTEL_REGISTRY_ID: &str = "OTEL";

/// A Resolved Telemetry Schema.
/// A Resolved Telemetry Schema is self-contained and doesn't contain any
/// external references to other schemas or semantic conventions.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedTelemetrySchema {
    /// Version of the file structure.
    pub file_format: String,
    /// Schema URL that this file is published at.
    pub schema_url: String,
    /// The ID of the registry that this schema belongs to.
    pub registry_id: String,
    /// The registry that this schema belongs to.
    pub registry: Registry,
    /// Catalog of unique items that are shared across multiple registries
    /// and signals.
    pub catalog: Catalog,
    /// Resource definition (only for application).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<Resource>,
    /// Definition of the instrumentation library for the instrumented application or library.
    /// Or none if the resolved telemetry schema represents a semantic convention registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrumentation_library: Option<InstrumentationLibrary>,
    /// The list of dependencies of the current instrumentation application or library.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<InstrumentationLibrary>,
    /// Definitions for each schema version in this family.
    /// Note: the ordering of versions is defined according to semver
    /// version number ordering rules.
    /// This section is described in more details in the OTEP 0152 and in a dedicated
    /// section below.
    /// <https://github.com/open-telemetry/oteps/blob/main/text/0152-telemetry-schemas.md>
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versions: Option<Versions>,
    /// The manifest of the registry.
    pub registry_manifest: Option<RegistryManifest>,
}

/// Statistics on a resolved telemetry schema.
#[derive(Debug, Serialize)]
#[must_use]
pub struct Stats {
    /// Statistics on each registry.
    pub registry_stats: Vec<registry::Stats>,
    /// Statistics on the catalog.
    pub catalog_stats: catalog::Stats,
}

impl ResolvedTelemetrySchema {
    /// Create a new resolved telemetry schema.
    pub fn new<S: AsRef<str>>(
        file_format: S,
        schema_url: S,
        registry_id: S,
        registry_url: S,
    ) -> Self {
        Self {
            file_format: file_format.as_ref().to_owned(),
            schema_url: schema_url.as_ref().to_owned(),
            registry_id: registry_id.as_ref().to_owned(),
            registry: Registry::new(registry_url),
            catalog: Catalog::default(),
            resource: None,
            instrumentation_library: None,
            dependencies: vec![],
            versions: None,
            registry_manifest: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn add_metric_group<const N: usize>(
        &mut self,
        group_id: &str,
        metric_name: &str,
        attrs: [Attribute; N],
        deprecated: Option<Deprecated>,
    ) {
        let attr_refs = self.catalog.add_attributes(attrs);
        self.registry.groups.push(Group {
            id: group_id.to_owned(),
            r#type: GroupType::Metric,
            brief: "".to_owned(),
            note: "".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: None,
            deprecated,
            name: Some(group_id.to_owned()),
            lineage: None,
            display_name: None,
            attributes: attr_refs,
            span_kind: None,
            events: vec![],
            metric_name: Some(metric_name.to_owned()),
            instrument: Some(weaver_semconv::group::InstrumentSpec::Gauge),
            unit: Some("{things}".to_owned()),
            body: None,
            annotations: None,
            entity_associations: vec![],
            value_type: Some(weaver_semconv::metric::MetricValueTypeSpec::Double),
        });
    }

    /// Adds a new attribute group to the schema.
    ///
    /// Note: This method is intended to be used for testing purposes only.
    #[cfg(test)]
    pub(crate) fn add_attribute_group<const N: usize>(
        &mut self,
        group_id: &str,
        attrs: [Attribute; N],
    ) {
        let attr_refs = self.catalog.add_attributes(attrs);
        self.registry.groups.push(Group {
            id: group_id.to_owned(),
            r#type: GroupType::AttributeGroup,
            brief: "".to_owned(),
            note: "".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: None,
            deprecated: None,
            name: Some(group_id.to_owned()),
            lineage: None,
            display_name: None,
            attributes: attr_refs,
            span_kind: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            body: None,
            annotations: None,
            entity_associations: vec![],
            value_type: None,
        });
    }

    /// Get the catalog of the resolved telemetry schema.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Compute statistics on the resolved telemetry schema.
    pub fn stats(&self) -> Stats {
        let registry_stats = vec![self.registry.stats()];
        Stats {
            registry_stats,
            catalog_stats: self.catalog.stats(),
        }
    }

    /// Get the attributes of the resolved telemetry schema.
    #[must_use]
    pub fn attribute_map(&self) -> HashMap<&str, &Attribute> {
        self.registry
            .groups
            .iter()
            .filter(|group| group.r#type == GroupType::AttributeGroup)
            .flat_map(|group| {
                group.attributes.iter().map(|attr_ref| {
                    // An attribute ref is a reference to an attribute in the catalog.
                    // Not finding the attribute in the catalog is a bug somewhere in
                    // the resolution process. So it's fine to panic here.
                    let attr = self
                        .catalog
                        .attribute(attr_ref)
                        .expect("Attribute ref not found in catalog. This is a bug.");
                    (attr.name.as_str(), attr)
                })
            })
            .collect()
    }

    /// Get the "registry" attributes of the resolved telemetry schema.
    ///
    /// Note: At the moment (2024-12-30), I don't know a better way to identify
    /// the "registry" attributes other than by checking if the group ID starts
    /// with "registry.".
    #[must_use]
    pub fn registry_attribute_map(&self) -> HashMap<&str, &Attribute> {
        self.registry
            .groups
            .iter()
            .filter(|group| group.is_registry_attribute_group())
            .flat_map(|group| {
                group.attributes.iter().map(|attr_ref| {
                    // An attribute ref is a reference to an attribute in the catalog.
                    // Not finding the attribute in the catalog is a bug somewhere in
                    // the resolution process. So it's fine to panic here.
                    let attr = self
                        .catalog
                        .attribute(attr_ref)
                        .expect("Attribute ref not found in catalog. This is a bug.");
                    (attr.name.as_str(), attr)
                })
            })
            .collect()
    }

    /// Get the groups of a specific type from the resolved telemetry schema.
    #[must_use]
    pub fn groups(&self, group_type: GroupType) -> HashMap<&str, &Group> {
        self.registry
            .groups
            .iter()
            .filter(|group| group.r#type == group_type)
            .map(|group| (group.id.as_str(), group))
            .collect()
    }

    /// Grab a specific type of group identified by the name (not id).
    #[must_use]
    pub fn groups_by_name(&self, group_type: GroupType) -> HashMap<&str, &Group> {
        self.registry
            .groups
            .iter()
            .filter(|group| group.r#type == group_type)
            .filter_map(|g| g.signal_name().map(|name| (name, g)))
            .collect()
    }

    /// Get the group by its ID.
    #[must_use]
    pub fn group(&self, group_id: &str) -> Option<&Group> {
        self.registry
            .groups
            .iter()
            .find(|group| group.id == group_id)
    }

    /// Generate a diff between the current schema (must be the most recent one)
    /// and a baseline schema.
    #[must_use]
    pub fn diff(&self, baseline_schema: &ResolvedTelemetrySchema) -> SchemaChanges {
        let mut changes = SchemaChanges::new();

        if let Some(ref manifest) = self.registry_manifest {
            changes.set_head_manifest(weaver_version::schema_changes::RegistryManifest {
                semconv_version: manifest.semconv_version.clone(),
            });
        }

        if let Some(ref manifest) = baseline_schema.registry_manifest {
            changes.set_baseline_manifest(weaver_version::schema_changes::RegistryManifest {
                semconv_version: manifest.semconv_version.clone(),
            });
        }

        // Attributes in the registry
        self.diff_attributes(baseline_schema, &mut changes);

        // Signals
        let latest_signals = self.groups_by_name(GroupType::Metric);
        let baseline_signals = baseline_schema.groups_by_name(GroupType::Metric);
        self.diff_signals(
            SchemaItemType::Metrics,
            &latest_signals,
            &baseline_signals,
            &mut changes,
        );
        let latest_signals = self.groups_by_name(GroupType::Event);
        let baseline_signals = baseline_schema.groups_by_name(GroupType::Event);
        self.diff_signals(
            SchemaItemType::Events,
            &latest_signals,
            &baseline_signals,
            &mut changes,
        );
        // Note: We cannot support spans here. Currently spans do not have a stable identifier to represent them.
        // This means `groups_by_name` never returns a group today.
        // See: https://github.com/open-telemetry/semantic-conventions/issues/2055
        let latest_signals = self.groups_by_name(GroupType::Span);
        let baseline_signals = baseline_schema.groups_by_name(GroupType::Span);
        self.diff_signals(
            SchemaItemType::Spans,
            &latest_signals,
            &baseline_signals,
            &mut changes,
        );
        let latest_signals = self.groups_by_name(GroupType::Entity);
        let baseline_signals = baseline_schema.groups_by_name(GroupType::Entity);
        self.diff_signals(
            SchemaItemType::Entities,
            &latest_signals,
            &baseline_signals,
            &mut changes,
        );

        changes
    }

    fn diff_attributes(
        &self,
        baseline_schema: &ResolvedTelemetrySchema,
        changes: &mut SchemaChanges,
    ) {
        let latest_attributes = self.registry_attribute_map();
        let baseline_attributes = baseline_schema.registry_attribute_map();

        // ToDo for future PR, process differences at the field level (not required for the schema update)

        // Collect all the information related to the attributes that have been
        // deprecated in the latest schema.
        for (attr_name, attr) in latest_attributes.iter() {
            let baseline_attr = baseline_attributes.get(attr_name);

            if let Some(baseline_attr) = baseline_attr {
                if let Some(deprecated) = attr.deprecated.as_ref() {
                    // is this a change from the baseline?
                    if let Some(baseline_deprecated) = baseline_attr.deprecated.as_ref() {
                        if deprecated == baseline_deprecated {
                            // This attribute was already deprecated in the baseline.
                            // We can skip it.
                            continue;
                        }
                    }

                    match deprecated {
                        Deprecated::Renamed {
                            renamed_to: rename_to,
                            note,
                        } => {
                            changes.add_change(
                                SchemaItemType::RegistryAttributes,
                                SchemaItemChange::Renamed {
                                    old_name: baseline_attr.name.clone(),
                                    new_name: rename_to.clone(),
                                    note: note.clone(),
                                },
                            );
                        }
                        Deprecated::Obsoleted { note } => {
                            changes.add_change(
                                SchemaItemType::RegistryAttributes,
                                SchemaItemChange::Obsoleted {
                                    name: attr.name.clone(),
                                    note: note.clone(),
                                },
                            );
                        }
                        Deprecated::Unspecified { note } | Deprecated::Uncategorized { note } => {
                            changes.add_change(
                                SchemaItemType::RegistryAttributes,
                                SchemaItemChange::Uncategorized {
                                    name: attr.name.clone(),
                                    note: note.clone(),
                                },
                            );
                        }
                    }
                }
            } else {
                changes.add_change(
                    SchemaItemType::RegistryAttributes,
                    SchemaItemChange::Added {
                        name: attr.name.clone(),
                    },
                );
            }
        }

        // Any attribute in the baseline schema that is not present in the latest schema
        // is considered removed.
        // Note: This should never occur if the registry evolution process is followed.
        // However, detecting this case is useful for identifying a violation of the process.
        for (attr_name, attr) in baseline_attributes.iter() {
            if !latest_attributes.contains_key(attr_name) {
                changes.add_change(
                    SchemaItemType::RegistryAttributes,
                    SchemaItemChange::Removed {
                        name: attr.name.clone(),
                    },
                );
            }
        }
    }

    fn diff_signals(
        &self,
        schema_item_type: SchemaItemType,
        latest_signals: &HashMap<&str, &Group>,
        baseline_signals: &HashMap<&str, &Group>,
        changes: &mut SchemaChanges,
    ) {
        // Collect all the information related to the signals that have been
        // deprecated in the latest schema.
        for (signal_name, group) in latest_signals.iter() {
            let baseline_group = baseline_signals.get(signal_name);

            if let Some(baseline_group) = baseline_group {
                if let Some(deprecated) = group.deprecated.as_ref() {
                    // is this a change from the baseline?
                    if let Some(baseline_deprecated) = baseline_group.deprecated.as_ref() {
                        if deprecated == baseline_deprecated {
                            continue;
                        }
                    }

                    match deprecated {
                        Deprecated::Renamed {
                            renamed_to: rename_to,
                            note,
                        } => {
                            changes.add_change(
                                schema_item_type,
                                SchemaItemChange::Renamed {
                                    old_name: (*signal_name).to_owned(),
                                    new_name: rename_to.clone(),
                                    note: note.clone(),
                                },
                            );
                        }
                        Deprecated::Obsoleted { note } => {
                            changes.add_change(
                                schema_item_type,
                                SchemaItemChange::Obsoleted {
                                    name: (*signal_name).to_owned(),
                                    note: note.clone(),
                                },
                            );
                        }
                        Deprecated::Unspecified { note } | Deprecated::Uncategorized { note } => {
                            changes.add_change(
                                schema_item_type,
                                SchemaItemChange::Uncategorized {
                                    name: (*signal_name).to_owned(),
                                    note: note.clone(),
                                },
                            );
                        }
                    }
                }
            } else {
                changes.add_change(
                    schema_item_type,
                    SchemaItemChange::Added {
                        name: (*signal_name).to_owned(),
                    },
                );
            }
        }

        // Any signal in the baseline schema that is not present in the latest schema
        // is considered removed.
        // Note: This should never occur if the registry evolution process is followed.
        // However, detecting this case is useful for identifying a violation of the process.
        for (signal_name, _) in baseline_signals.iter() {
            if !latest_signals.contains_key(signal_name) {
                changes.add_change(
                    schema_item_type,
                    SchemaItemChange::Removed {
                        name: (*signal_name).to_owned(),
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::attribute::Attribute;
    use crate::ResolvedTelemetrySchema;
    use schemars::schema_for;
    use serde_json::to_string_pretty;
    use weaver_semconv::deprecated::Deprecated;
    use weaver_version::schema_changes::{SchemaItemChange, SchemaItemType};

    #[test]
    fn test_json_schema_gen() {
        // Ensure the JSON schema can be generated for the ResolvedTelemetrySchema
        let schema = schema_for!(ResolvedTelemetrySchema);

        // Ensure the schema can be serialized to a string
        assert!(to_string_pretty(&schema).is_ok());
    }

    #[test]
    fn no_diff() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::int("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );

        let changes = prior_schema.diff(&prior_schema);
        assert!(changes.is_empty());
    }

    #[test]
    fn detect_2_added_registry_attributes() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
            ],
        );

        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::int("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );

        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 2);
        assert_eq!(changes.count_registry_attribute_changes(), 2);
        assert_eq!(changes.count_added_registry_attributes(), 2);
    }

    #[test]
    fn detect_2_deprecated_registry_attributes() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::int("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
                Attribute::double("attr5", "brief5", "note5").deprecated(Deprecated::Obsoleted {
                    note: "".to_owned(),
                }),
            ],
        );

        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2")
                    .deprecated(Deprecated::Obsoleted {
                        note: "This attribute is deprecated (deprecated).".to_owned(),
                    })
                    .brief("This attribute is deprecated (brief)."),
                Attribute::int("attr3", "brief3", "note3")
                    .deprecated(Deprecated::Obsoleted {
                        note: "".to_owned(),
                    })
                    .brief("This attribute is deprecated."),
                Attribute::double("attr4", "brief4", "note4"),
                Attribute::double("attr5", "brief5", "note5").deprecated(Deprecated::Obsoleted {
                    note: "".to_owned(),
                }),
            ],
        );

        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 2);
        assert_eq!(changes.count_registry_attribute_changes(), 2);
        assert_eq!(changes.count_obsoleted_registry_attributes(), 2);
        for attr_change in changes
            .changes_by_type(SchemaItemType::RegistryAttributes)
            .unwrap()
        {
            match attr_change {
                SchemaItemChange::Obsoleted { name, note } => {
                    if name == "attr2" {
                        assert_eq!(note, "This attribute is deprecated (deprecated).");
                    } else if name == "attr3" {
                        assert_eq!(note, "");
                    } else {
                        panic!("Unexpected attribute name.");
                    }
                }
                _ => panic!("Unexpected change type."),
            }
        }
    }

    #[test]
    fn detect_2_renamed_registry_attributes() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::int("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );

        // 2 new attributes are added: attr2_bis and attr3_bis
        // attr2 is renamed attr2_bis
        // attr3 is renamed attr3_bis
        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2").deprecated(Deprecated::Renamed {
                    renamed_to: "attr2_bis".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::int("attr3", "brief3", "note3").deprecated(Deprecated::Renamed {
                    renamed_to: "attr3_bis".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );
        latest_schema.add_attribute_group(
            "registry.group2",
            [
                Attribute::boolean("attr2_bis", "brief1", "note1"),
                Attribute::boolean("attr3_bis", "brief1", "note1"),
            ],
        );

        let changes = latest_schema.diff(&prior_schema);
        dbg!(&changes);
        assert_eq!(changes.count_changes(), 4);
        assert_eq!(changes.count_registry_attribute_changes(), 4);
        assert_eq!(changes.count_renamed_registry_attributes(), 2);
        assert_eq!(changes.count_added_registry_attributes(), 2);
    }

    #[test]
    fn detect_2_attributes_renamed_to_the_same_existing_attribute() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::string("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );
        prior_schema.add_attribute_group("group2", [Attribute::string("attr5", "brief", "note")]);

        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2").deprecated(Deprecated::Renamed {
                    renamed_to: "attr5".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::int("attr3", "brief3", "note3").deprecated(Deprecated::Renamed {
                    renamed_to: "attr5".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );
        latest_schema.add_attribute_group("group2", [Attribute::string("attr5", "brief", "note")]);

        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 2);
        assert_eq!(changes.count_registry_attribute_changes(), 2);
        assert_eq!(changes.count_renamed_registry_attributes(), 2);
        dbg!(&changes);
    }

    #[test]
    fn detect_2_attributes_renamed_to_the_same_new_attribute() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::string("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );

        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2").deprecated(Deprecated::Renamed {
                    renamed_to: "attr5".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::int("attr3", "brief3", "note3").deprecated(Deprecated::Renamed {
                    renamed_to: "attr5".to_owned(),
                    note: "".to_owned(),
                }),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );
        latest_schema.add_attribute_group(
            "registry.group2",
            [Attribute::string("attr5", "brief", "note")],
        );

        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 3);
        assert_eq!(changes.count_registry_attribute_changes(), 3);
        assert_eq!(changes.count_renamed_registry_attributes(), 2);
        assert_eq!(changes.count_added_registry_attributes(), 1);
        dbg!(&changes);
    }

    /// In normal situation this should never happen based on the registry evolution process.
    /// However, detecting this case is useful for identifying a violation of the process.
    #[test]
    fn detect_2_removed_attributes() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        prior_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
                Attribute::int("attr3", "brief3", "note3"),
                Attribute::double("attr4", "brief4", "note4"),
            ],
        );

        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2"),
            ],
        );

        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 2);
        assert_eq!(changes.count_registry_attribute_changes(), 2);
        assert_eq!(changes.count_removed_registry_attributes(), 2);
    }

    // TODO add many more group diff checks for various capabilities.
    #[test]
    fn detect_metric_name_change() {
        let mut prior_schema = ResolvedTelemetrySchema::new("1.0", "test/base_version", "", "");
        prior_schema.add_metric_group("metrics.cpu.time", "cpu.time", [], None);
        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "test/new_version", "", "");
        latest_schema.add_metric_group(
            "metrics.cpu.time",
            "cpu.time",
            [],
            Some(Deprecated::Renamed {
                renamed_to: "system.cpu.time".to_owned(),
                note: "Replaced by `system.cpu.utilization`".to_owned(),
            }),
        );
        latest_schema.add_metric_group("metrics.system.cpu.time", "system.cpu.time", [], None);
        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 2);
        assert_eq!(changes.count_metric_changes(), 2);
        let Some(mcs) = changes.changes_by_type(SchemaItemType::Metrics) else {
            panic!("No metric changes in {:?}", changes)
        };

        let Some(SchemaItemChange::Renamed {
            old_name,
            new_name,
            note,
        }) = mcs
            .iter()
            .find(|change| matches!(change, &SchemaItemChange::Renamed { .. }))
        else {
            panic!("No rename change found in: {:?}", mcs);
        };
        assert_eq!(old_name, "cpu.time");
        assert_eq!(new_name, "system.cpu.time");
        assert_eq!(note, "Replaced by `system.cpu.utilization`");

        let Some(SchemaItemChange::Added { name }) = mcs
            .iter()
            .find(|change| matches!(change, &SchemaItemChange::Added { .. }))
        else {
            panic!("No added change found in: {:?}", mcs);
        };
        assert_eq!(name, "system.cpu.time");
    }
}
