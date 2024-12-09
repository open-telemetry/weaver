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
use std::collections::{HashMap, HashSet};
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

    /// Get the metrics of the resolved telemetry schema.
    #[must_use]
    pub fn groups(&self, group_type: GroupType) -> HashMap<String, &Group> {
        self.registry
            .groups
            .iter()
            .filter(|group| group.r#type == group_type)
            .map(|group| match group_type {
                GroupType::AttributeGroup
                | GroupType::Event
                | GroupType::Span
                | GroupType::Resource
                | GroupType::MetricGroup
                | GroupType::Scope => (group.name.clone().unwrap_or_default(), group),
                GroupType::Metric => (group.metric_name.clone().unwrap_or_default(), group),
            })
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
        let latest_attributes = self.attribute_map();
        let baseline_attributes = baseline_schema.attribute_map();

        // A map of attributes that have been renamed to a new attribute.
        // The key is the new name of the attribute, and the value is a set of old names.
        // The key may refer to an existing attribute in the baseline schema or
        // a new attribute in the latest schema.
        let mut renamed_attributes = HashMap::new();

        // ToDo for future PR, process differences at the field level (not required for the schema update)

        // Collect all the information related to the attributes that have been
        // deprecated in the latest schema.
        for (attr_name, attr) in latest_attributes.iter() {
            if let Some(deprecated) = attr.deprecated.as_ref() {
                match deprecated {
                    Deprecated::Renamed {
                        new_name: rename_to,
                        ..
                    } => {
                        // Insert the old name into the set of old names
                        // for the new name (rename_to).
                        _ = renamed_attributes
                            .entry(rename_to.as_str())
                            .or_insert_with(HashSet::new)
                            .insert(*attr_name);
                    }
                    Deprecated::Deprecated { .. } => {
                        changes.add_change(
                            SchemaItemType::Attributes,
                            SchemaItemChange::Deprecated {
                                name: attr.name.clone(),
                                note: deprecated.to_string(),
                            },
                        );
                    }
                }
            }
        }

        // Based on the analysis of deprecated fields conducted earlier, we can
        // now distinguish between:
        // - an attribute created to give a new name to an existing attribute or
        //   to unify several attributes into a single one,
        // - an attribute created to represent something new.
        for (attr_name, attr) in latest_attributes.iter() {
            if !baseline_attributes.contains_key(attr_name) {
                // The attribute in the latest schema does not exist in the baseline schema.
                // This attribute may be referenced in the deprecated field of another
                // attribute, indicating that it is a replacement attribute intended to rename
                // one or more existing attributes.
                // If it is not referenced in the deprecated field of another attribute, then
                // it is an entirely new attribute that did not previously exist.
                if let Some(old_names) = renamed_attributes.remove(attr_name) {
                    // The new attribute is identified as a replacement attribute based
                    // on the deprecated metadata.
                    changes.add_change(
                        SchemaItemType::Attributes,
                        SchemaItemChange::RenamedToNew {
                            old_names: old_names.iter().map(|n| (*n).to_owned()).collect(),
                            new_name: attr.name.clone(),
                        },
                    );
                } else {
                    // The new attribute is identified as a new attribute not related to
                    // any previous attributes in the baseline schema.
                    changes.add_change(
                        SchemaItemType::Attributes,
                        SchemaItemChange::Added {
                            name: attr.name.clone(),
                        },
                    );
                }
            }
        }

        // Any attribute in the baseline schema that is not present in the latest schema
        // is considered removed.
        // Note: This should never occur if the registry evolution process is followed.
        // However, detecting this case is useful for identifying a violation of the process.
        for (attr_name, attr) in baseline_attributes.iter() {
            if !latest_attributes.contains_key(attr_name) {
                changes.add_change(
                    SchemaItemType::Attributes,
                    SchemaItemChange::Removed {
                        name: attr.name.clone(),
                    },
                );
            }
        }

        // The attribute names that remain in the list `renamed_attributes` are those
        // present in both versions of the same schema. They represent cases where
        // attributes have been renamed to an already existing attribute.
        for (new_name, old_names) in renamed_attributes.iter() {
            changes.add_change(
                SchemaItemType::Attributes,
                SchemaItemChange::RenamedToExisting {
                    old_names: old_names.iter().map(|n| (*n).to_owned()).collect(),
                    current_name: (*new_name).to_owned(),
                },
            );
        }
    }

    fn diff_signals(
        &self,
        schema_item_type: SchemaItemType,
        latest_signals: &HashMap<String, &Group>,
        baseline_signals: &HashMap<String, &Group>,
        changes: &mut SchemaChanges,
    ) {
        /// Get the name of the provided group based on the given schema item type.
        fn group_name(schema_item_type: SchemaItemType, group: &Group) -> String {
            match schema_item_type {
                SchemaItemType::Attributes
                | SchemaItemType::Events
                | SchemaItemType::Spans
                | SchemaItemType::Resources => group.name.clone().unwrap_or_default(),
                SchemaItemType::Metrics => group.metric_name.clone().unwrap_or_default(),
            }
        }

        // A map of signal groups that have been renamed to a new signal of same type.
        // The key is the new name of the signal, and the value is a set of old names.
        // The key may refer to an existing signal in the baseline schema or
        // a new signal in the latest schema.
        let mut renamed_signals: HashMap<String, Vec<&Group>> = HashMap::new();

        // Collect all the information related to the signals that have been
        // deprecated in the latest schema.
        for (signal_name, group) in latest_signals.iter() {
            if let Some(deprecated) = group.deprecated.as_ref() {
                match deprecated {
                    Deprecated::Renamed {
                        new_name: rename_to,
                        ..
                    } => {
                        // Insert the deprecated signal into the set of renamed/deprecated signals
                        // for the new name (rename_to).
                        renamed_signals
                            .entry(rename_to.clone())
                            .or_default()
                            .push(group);
                    }
                    Deprecated::Deprecated { .. } => {
                        changes.add_change(
                            schema_item_type,
                            SchemaItemChange::Deprecated {
                                name: signal_name.clone(),
                                note: deprecated.to_string(),
                            },
                        );
                    }
                }
            }
        }

        // Based on the analysis of deprecated fields conducted earlier, we can
        // now distinguish between:
        // - a signal created to give a new name to an existing signal or
        //   to unify several signals into a single one,
        // - a signal created to represent something new.
        for (signal_name, _) in latest_signals.iter() {
            if !baseline_signals.contains_key(signal_name) {
                // The signal in the latest schema does not exist in the baseline schema.
                // This signal may be referenced in the deprecated field of another
                // signal, indicating that it is a replacement signal intended to rename
                // one or more existing signals.
                // If it is not referenced in the deprecated field of another signal, then
                // it is an entirely new signal that did not previously exist.
                if let Some(old_groups) = renamed_signals.remove(signal_name) {
                    // The new signal is identified as a replacement signal based
                    // on the deprecated metadata.
                    changes.add_change(
                        schema_item_type,
                        SchemaItemChange::RenamedToNew {
                            old_names: old_groups
                                .iter()
                                .map(|n| group_name(schema_item_type, n))
                                .collect(),
                            new_name: signal_name.clone(),
                        },
                    );
                } else {
                    // The new signal is identified as a new signal not related to
                    // any previous signals in the baseline schema.
                    changes.add_change(
                        schema_item_type,
                        SchemaItemChange::Added {
                            name: signal_name.clone(),
                        },
                    );
                }
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
                        name: signal_name.clone(),
                    },
                );
            }
        }

        // The signal names that remain in the list `renamed_signals` are those
        // present in both versions of the same schema. They represent cases where
        // signals have been renamed to an already existing signal.
        for (new_name, old_groups) in renamed_signals.iter() {
            changes.add_change(
                schema_item_type,
                SchemaItemChange::RenamedToExisting {
                    old_names: old_groups
                        .iter()
                        .map(|n| group_name(schema_item_type, n))
                        .collect(),
                    current_name: (*new_name).to_owned(),
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ResolvedTelemetrySchema;
    use schemars::schema_for;
    use serde_json::to_string_pretty;

    #[test]
    fn test_json_schema_gen() {
        // Ensure the JSON schema can be generated for the ResolvedTelemetrySchema
        let schema = schema_for!(ResolvedTelemetrySchema);

        // Ensure the schema can be serialized to a string
        assert!(to_string_pretty(&schema).is_ok());
    }

    // ToDo LQ add tests for the diff method
}
