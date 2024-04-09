// SPDX-License-Identifier: Apache-2.0

//! Definition of a policy violation.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// Enum representing the different types of violations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Violation {
    /// A violation related to semantic convention attributes.
    SemconvAttribute {
        /// The ID of the policy violation.
        id: String,
        /// The category of the policy violation.
        category: String,
        /// The semconv group where the violation occurred.
        group: String,
        /// The semconv attribute where the violation occurred.
        attr: String,
    },
}

impl Display for Violation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Violation::SemconvAttribute {
                id,
                category,
                group,
                attr,
            } => {
                write!(
                    f,
                    "id={}, category={}, group={}, attr={}",
                    id, category, group, attr
                )
            }
        }
    }
}

impl Violation {
    /// Returns the violation id.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Violation::SemconvAttribute { id, .. } => id,
        }
    }
}
