// SPDX-License-Identifier: Apache-2.0

//! Stability specification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A migration specification
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationSpec {

    /// An optional brief to be added alongside link to migration guide
    pub brief: Option<String>,
    /// An optional note to be added to the migration guide
    pub note: Option<String>,
    /// The version which the migration guide should guide a user to.
    pub target: String,
}

impl MigrationSpec {
    /// returns the brief of the migration guide
    #[must_use]
    fn brief(&self) -> &Option<String> {
        &self.brief
    }
    /// returns the note of the migration guide
    #[must_use]
    fn note(&self) -> &Option<String> {
        &self.note
    }
    /// returns the target of the migration guide 
    #[must_use]
    fn target(&self) -> &String {
        &self.target
    }
}
