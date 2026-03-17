// SPDX-License-Identifier: Apache-2.0

//! The provenance of a semantic convention specification file.

use crate::schema_url::SchemaUrl;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// The provenance a semantic convention specification file.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Provenance {
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
    pub path: String,
}

impl Display for Provenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.schema_url, self.path)
    }
}

impl Provenance {
    /// Creates a new `Provenance` instance.
    #[must_use]
    pub fn new(schema_url: SchemaUrl, path: &str) -> Self {
        Provenance {
            schema_url,
            path: path.replace('\\', "/"),
        }
    }
}
