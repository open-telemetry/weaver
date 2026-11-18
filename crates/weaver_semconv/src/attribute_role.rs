// SPDX-License-Identifier: Apache-2.0

//! Attribute role definitions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The different roles for attributes in groups.
#[derive(
    Serialize, Deserialize, Debug, Default, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum AttributeRole {
    /// The attribute is considered identifying for the signal it is associated with.
    #[default]
    Identifying,
    /// The attribute is considered descriptive for the signal it is associated with.
    Descriptive,
}

impl AttributeRole {
    /// Returns `true` if this role is [`Self::Identifying`].
    #[must_use]
    pub fn is_identifying(&self) -> bool {
        matches!(self, Self::Identifying)
    }
}
