//! Provenance for v2 published schemas.

use std::fmt::Display;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{provenance::Provenance, schema_url::SchemaUrl};

/// Provenance for a signal or attribute in v2 published schema.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublishedProvenance {
    /// The schema URL where this was specified.
    ///
    /// The Schema url contains the registry id and the version of the schema.
    /// It can be used to detect conflicts or resolve multiple "ids" existing across
    /// dependency chains but being the same thing, conceptually.
    pub schema_url: SchemaUrl,

    /// The path to the specification file.
    ///
    /// This is the path is only available *locally*. When publishing resolved schemas,
    /// this field is not included.
    #[serde(skip)]
    pub path: String,
}

impl Display for PublishedProvenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.schema_url.name(), self.path)
    }
}

impl From<Provenance> for PublishedProvenance {
    fn from(value: Provenance) -> Self {
        PublishedProvenance {
            schema_url: value.schema_url,
            path: value.path,
        }
    }
}
