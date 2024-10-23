// SPDX-License-Identifier: Apache-2.0

//! Define the concept of Resolved Telemetry Schema.
//! A Resolved Telemetry Schema is self-contained and doesn't contain any
//! external references to other schemas or semantic conventions.

use std::collections::HashMap;
use crate::catalog::Catalog;
use crate::instrumentation_library::InstrumentationLibrary;
use crate::registry::Registry;
use crate::resource::Resource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::group::GroupType;
use weaver_version::Versions;
use crate::attribute::Attribute;

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

impl ResolvedTelemetrySchema {
    /// Get the catalog of the resolved telemetry schema.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Compute statistics on the resolved telemetry schema.
    pub fn stats(&self) -> Stats {
        let mut registry_stats = Vec::new();
        registry_stats.push(self.registry.stats());
        Stats {
            registry_stats,
            catalog_stats: self.catalog.stats(),
        }
    }

    /// Get the attributes of the resolved telemetry schema.
    pub fn attribute_map(&self) -> HashMap<&str, &Attribute> {
        self.registry.groups.iter()
            .filter(|group| group.r#type == GroupType::AttributeGroup)
            .flat_map(|group| group.attributes.iter()
                .map(|attr_ref| {
                    let attr = self.catalog.attribute(attr_ref).unwrap();
                    (attr.name.as_str(), attr)
                }))
            .collect()
    }

    /// Generate a diff between the current schema and a baseline schema.
    pub fn diff(&self, baseline_schema: &ResolvedTelemetrySchema) {
        let attributes = self.attribute_map();
        let baseline_attributes = baseline_schema.attribute_map();
        
        // Detect new attributes
        for (name, attr) in attributes.iter() {
            if !baseline_attributes.contains_key(name) {
                println!("New attribute: {}", attr.name);
            }
        }
        
        // Detect removed attributes
        for (name, attr) in baseline_attributes.iter() {
            if !attributes.contains_key(name) {
                println!("Removed attribute: {}", attr.name);
            }
        }
        
        // Detect changed attributes
        for (name, attr) in attributes.iter() {
            if let Some(baseline_attr) = baseline_attributes.get(name) {
                if attr != baseline_attr {
                    println!("Changed attribute: {}", attr.name);
                }
            }
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
