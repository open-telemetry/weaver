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
}
