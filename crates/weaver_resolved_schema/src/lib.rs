// SPDX-License-Identifier: Apache-2.0

//! Define the concept of Resolved Telemetry Schema.
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

/// The set of possible change types that can be observed between
/// two versions of a schema.
/// ToDo Is it the right place to specify this enum
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum SchemaChange {
    /// An attribute has been added.
    AddedAttribute {
        /// The name of the added attribute.
        name: String,
    },
    /// One or more attributes have been renamed into a new attribute.
    RenamedToNewAttribute {
        /// The old names of the attributes that have been renamed.
        old_names: HashSet<String>,
        /// The new name of the attributes that have been renamed.
        new_name: String,
    },
    /// One or more attributes have been renamed into an existing attribute.
    RenamedToExistingAttribute {
        /// The old names of the attributes that have been renamed.
        old_names: HashSet<String>,
        /// The current name of the attributes that have been renamed.
        current_name: String,
    },
    /// Deprecated attribute.
    DeprecatedAttribute {
        /// The name of the deprecated attribute.
        name: String,
        /// The deprecation note.
        note: String,
    },
    /// An attribute has been removed.
    RemovedAttribute {
        /// The name of the removed attribute.
        name: String,
    },

    /// A metric has been added.
    AddedMetric {
        /// The name of the added metric.
        name: String,
    },
    /// One or more metrics have been renamed into a new metric.
    RenamedToNewMetric {
        /// The old names of the metrics that have been renamed.
        old_names: HashSet<String>,
        /// The new name of the metrics that have been renamed.
        new_name: String,
    },
    /// One or more metrics have been renamed into an existing metric.
    RenamedToExistingMetric {
        /// The old names of the metrics that have been renamed.
        old_names: HashSet<String>,
        /// The current name of the metrics that have been renamed.
        current_name: String,
    },
    /// Deprecated metric.
    DeprecatedMetric {
        /// The name of the deprecated metric.
        name: String,
        /// The deprecation note.
        note: String,
    },
    /// A metric has been removed.
    RemovedMetric {
        /// The name of the removed metric.
        name: String,
    },
}

impl ResolvedTelemetrySchema {
    /// Get the catalog of the resolved telemetry schema.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Compute statistics on the resolved telemetry schema.
    pub fn stats(&self) -> Stats {
        let registry_stats = vec![
            self.registry.stats()
        ];
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
    pub fn metric_map(&self) -> HashMap<String, &Group> {
        self.registry
            .groups
            .iter()
            .filter(|group| group.r#type == GroupType::Metric)
            .map(|group| (group.metric_name.clone().unwrap_or_default(), group))
            .collect()
    }

    /// Generate a diff between the current schema and a baseline schema.
    #[must_use]
    pub fn diff(&self, baseline_schema: &ResolvedTelemetrySchema) -> Vec<SchemaChange> {
        let mut changes = Vec::new();

        self.diff_attributes(baseline_schema, &mut changes);
        self.diff_metrics(baseline_schema, &mut changes);

        changes
    }

    fn diff_attributes(
        &self,
        baseline_schema: &ResolvedTelemetrySchema,
        changes: &mut Vec<SchemaChange>,
    ) {
        let latest_attributes = self.attribute_map();
        let baseline_attributes = baseline_schema.attribute_map();

        // A map of attributes that have been renamed to a new attribute.
        // The key is the new name of the attribute, and the value is a set of old names.
        // The key may refer to an existing attribute in the baseline schema or
        // a new attribute in the latest schema.
        let mut renamed_attributes = HashMap::new();

        // ToDo process differences at the field level ?
        // ToDo create a struct SchemaChanges containing all the changes and exposing a stats method

        // Collect all the information related to the attributes that have been
        // deprecated in the latest schema.
        for (attr_name, attr) in latest_attributes.iter() {
            if let Some(deprecated) = attr.deprecated.as_ref() {
                if !baseline_attributes.contains_key(attr_name) {
                    println!("We should never see a new attribute with a deprecated field! Attribute: {}", attr_name);
                }
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
                        changes.push(SchemaChange::DeprecatedAttribute {
                            name: attr.name.clone(),
                            note: deprecated.to_string(),
                        });
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
                    changes.push(SchemaChange::RenamedToNewAttribute {
                        old_names: old_names.iter().map(|n| (*n).to_owned()).collect(),
                        new_name: attr.name.clone(),
                    });
                } else {
                    // The new attribute is identified as a new attribute not related to
                    // any previous attributes in the baseline schema.
                    changes.push(SchemaChange::AddedAttribute {
                        name: attr.name.clone(),
                    });
                }
            }
        }

        // Any attribute in the baseline schema that is not present in the latest schema
        // is considered removed.
        // Note: This should never occur if the registry evolution process is followed.
        // However, detecting this case is useful for identifying a violation of the process.
        for (attr_name, attr) in baseline_attributes.iter() {
            if !latest_attributes.contains_key(attr_name) {
                changes.push(SchemaChange::RemovedAttribute {
                    name: attr.name.clone(),
                });
            }
        }

        // The attribute names that remain in the list `renamed_attributes` are those
        // present in both versions of the same schema. They represent cases where
        // attributes have been renamed to an already existing attribute.
        for (new_name, old_names) in renamed_attributes.iter() {
            changes.push(SchemaChange::RenamedToExistingAttribute {
                old_names: old_names.iter().map(|n| (*n).to_owned()).collect(),
                current_name: (*new_name).to_owned(),
            });
        }
    }

    fn diff_metrics(
        &self,
        baseline_schema: &ResolvedTelemetrySchema,
        changes: &mut Vec<SchemaChange>,
    ) {
        let metrics = self.metric_map();
        let baseline_metrics = baseline_schema.metric_map();

        // Detect changed metrics and, based on the deprecated field,
        // build maps of metrics that have been renamed or deprecated.
        let mut renamed_metrics: HashMap<String, Vec<&Group>> = HashMap::new();
        for (name, metric) in metrics.iter() {
            if let Some(baseline_metric) = baseline_metrics.get(name) {
                if metric != baseline_metric {
                    if let Some(deprecated) = metric.deprecated.as_ref() {
                        match deprecated {
                            Deprecated::Renamed {
                                new_name: rename_to,
                                ..
                            } => {
                                // Insert the old name into the vec of old names
                                // for the new name (rename_to).
                                renamed_metrics
                                    .entry(rename_to.clone())
                                    .or_default()
                                    .push(metric);
                            }
                            Deprecated::Deprecated { .. } => {
                                changes.push(SchemaChange::DeprecatedMetric {
                                    name: name.clone(),
                                    note: deprecated.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Detect new or renamed metrics
        for (name, _) in metrics.iter() {
            if !baseline_metrics.contains_key(name) {
                // The metric in the ref_schema is identified as a new metric.
                // A metric is considered new for the following reasons:
                // - It is an entirely new metric that did not exist before.
                // - It is a replacement metric intended to rename one or more existing metrics.
                if let Some(old_groups) = renamed_metrics.remove(name) {
                    changes.push(SchemaChange::RenamedToNewMetric {
                        old_names: old_groups
                            .iter()
                            .map(|n| n.metric_name.clone().unwrap_or_default())
                            .collect(),
                        new_name: name.clone(),
                    });
                } else {
                    changes.push(SchemaChange::AddedMetric { name: name.clone() });
                }
            }
        }

        // Detect removed metrics
        for (name, _) in baseline_metrics.iter() {
            if !metrics.contains_key(name) {
                changes.push(SchemaChange::RemovedMetric { name: name.clone() });
            }
        }

        // All remaining metrics in renamed_metrics are renamed to an existing attribute
        for (new_name, old_groups) in renamed_metrics.iter() {
            changes.push(SchemaChange::RenamedToExistingMetric {
                old_names: old_groups
                    .iter()
                    .map(|n| n.metric_name.clone().unwrap_or_default())
                    .collect(),
                current_name: (*new_name).to_owned(),
            });
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
}
