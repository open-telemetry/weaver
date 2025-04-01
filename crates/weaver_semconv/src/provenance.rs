// SPDX-License-Identifier: Apache-2.0

//! The provenance of a semantic convention specification file.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::sync::Arc;

/// The provenance a semantic convention specification file.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Provenance {
    /// The registry id containing the specification file.
    /// A registry id is an identifier defined in the `registry_manifest.yaml` file.
    pub registry_id: Arc<str>,

    /// The path to the specification file.
    pub path: String,
}

impl Display for Provenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.registry_id, self.path)
    }
}

impl Provenance {
    /// Creates a new `Provenance` instance.
    #[must_use]
    pub fn new(registry_id: &str, path: &str) -> Self {
        Provenance {
            registry_id: Arc::from(registry_id),
            path: path.to_owned(),
        }
    }

    /// Creates an undefined `Provenance` instance.
    #[must_use]
    pub fn undefined() -> Self {
        Provenance {
            registry_id: Arc::from("undefined"),
            path: "undefined".to_owned(),
        }
    }
}
