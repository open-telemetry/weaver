// SPDX-License-Identifier: Apache-2.0

//! The provenance of a semantic convention attribute or signal in forge.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::schema_url::SchemaUrl;

/// The provenance of a semantic convention attribute or signal in forge.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default, Hash, Eq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Provenance {
    /// The dependency that defined this attribute or signal.
    ///
    /// Empty if the attribute or signal is not from a dependency.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub source: Option<SchemaUrl>,

    /// The path to the file that specified this attribute or signal.
    ///
    /// Empty if the attribute or signal is from a dependency.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Provenance {
    /// Returns true if this provenance is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.source.is_none() && self.path.is_none()
    }

    /// Turns a resolved provenance into the materialized one.
    ///
    /// `deps` is the dependency list of the schema the provenance came from,
    /// which is the table its `DependencyRef` indexes.
    #[must_use]
    pub fn from_resolved(
        provenance: &weaver_resolved_schema::v2::provenance::Provenance,
        deps: &[SchemaUrl],
    ) -> Self {
        Provenance {
            source: provenance
                .source
                .and_then(|dep_ref| deps.get(dep_ref.0 as usize).cloned()),
            path: (!provenance.path.is_empty()).then(|| provenance.path.clone()),
        }
    }
}
