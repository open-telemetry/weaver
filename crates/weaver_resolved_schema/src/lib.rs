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
            constraints: vec![],
            unit: None,
            body: None,
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
        let latest_signals = self.groups(GroupType::Metric);
        let baseline_signals = baseline_schema.groups(GroupType::Metric);
        self.diff_signals(
            SchemaItemType::Metrics,
            &latest_signals,
            &baseline_signals,
            &mut changes,
        );
        let latest_signals = self.groups(GroupType::Event);
        let baseline_signals = baseline_schema.groups(GroupType::Event);
        self.diff_signals(
            SchemaItemType::Events,
            &latest_signals,
            &baseline_signals,
            &mut changes,
        );
        let latest_signals = self.groups(GroupType::Span);
        let baseline_signals = baseline_schema.groups(GroupType::Span);
        self.diff_signals(
            SchemaItemType::Spans,
            &latest_signals,
            &baseline_signals,
            &mut changes,
        );
        let latest_signals = self.groups(GroupType::Resource);
        let baseline_signals = baseline_schema.groups(GroupType::Resource);
        self.diff_signals(
            SchemaItemType::Resources,
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
                            ..
                        } => {
                            changes.add_change(
                                SchemaItemType::RegistryAttributes,
                                SchemaItemChange::Renamed {
                                    old_name: baseline_attr.name.clone(),
                                    new_name: rename_to.clone(),
                                    note: attr.brief.clone(),
                                },
                            );
                        }
                        Deprecated::Obsoleted { .. } => {
                            changes.add_change(
                                SchemaItemType::RegistryAttributes,
                                SchemaItemChange::Obsoleted {
                                    name: attr.name.clone(),
                                    note: attr.brief.clone(),
                                },
                            );
                        }
                        Deprecated::Uncategorized => {
                            changes.add_change(
                                SchemaItemType::RegistryAttributes,
                                SchemaItemChange::Uncategorized {
                                    note: attr.brief.clone(),
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
                            ..
                        } => {
                            changes.add_change(
                                schema_item_type,
                                SchemaItemChange::Renamed {
                                    old_name: baseline_group.name.clone().expect("The name should be present since the baseline group was retrieved using it."),
                                    new_name: rename_to.clone(),
                                    note: group.brief.clone(),
                                },
                            );
                        }
                        Deprecated::Obsoleted { .. } => {
                            changes.add_change(
                                schema_item_type,
                                SchemaItemChange::Obsoleted {
                                    name: (*signal_name).to_owned(),
                                    note: deprecated.to_string(),
                                },
                            );
                        }
                        Deprecated::Uncategorized => {
                            changes.add_change(
                                schema_item_type,
                                SchemaItemChange::Uncategorized {
                                    note: deprecated.to_string(),
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
                Attribute::double("attr5", "brief5", "note5").deprecated(Deprecated::Obsoleted),
            ],
        );

        let mut latest_schema = ResolvedTelemetrySchema::new("1.0", "", "", "");
        latest_schema.add_attribute_group(
            "registry.group1",
            [
                Attribute::boolean("attr1", "brief1", "note1"),
                Attribute::string("attr2", "brief2", "note2")
                    .deprecated(Deprecated::Obsoleted)
                    .note("This attribute is deprecated."),
                Attribute::int("attr3", "brief3", "note3")
                    .deprecated(Deprecated::Obsoleted)
                    .note("This attribute is deprecated."),
                Attribute::double("attr4", "brief4", "note4"),
                Attribute::double("attr5", "brief5", "note5").deprecated(Deprecated::Obsoleted),
            ],
        );

        let changes = latest_schema.diff(&prior_schema);
        assert_eq!(changes.count_changes(), 2);
        assert_eq!(changes.count_registry_attribute_changes(), 2);
        assert_eq!(changes.count_deprecated_registry_attributes(), 2);
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
                }),
                Attribute::int("attr3", "brief3", "note3").deprecated(Deprecated::Renamed {
                    renamed_to: "attr3_bis".to_owned(),
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
                }),
                Attribute::int("attr3", "brief3", "note3").deprecated(Deprecated::Renamed {
                    renamed_to: "attr5".to_owned(),
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
                }),
                Attribute::int("attr3", "brief3", "note3").deprecated(Deprecated::Renamed {
                    renamed_to: "attr5".to_owned(),
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
}
