// SPDX-License-Identifier: Apache-2.0

//! Definition of a policy violation.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::{Display, Formatter};

/// Enum representing the different types of violations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
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
    /// Advice related to a policy violation.
    Advice(Advice),
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
            Violation::Advice(Advice {
                advice_type: r#type,
                value,
                message,
                advice_level,
            }) => {
                write!(
                    f,
                    "type={}, value={}, message={}, advice_level={:?}",
                    r#type, value, message, advice_level
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
            Violation::Advice(Advice {
                advice_type: r#type,
                ..
            }) => r#type,
        }
    }
}

/// The level of an advice
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, PartialOrd, Ord, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AdviceLevel {
    /// Useful context without action needed
    Information,
    /// Suggested change that would improve things
    Improvement,
    /// Something that breaks compliance rules
    Violation,
}

/// Represents a live check advice
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Advice {
    /// The type of advice e.g. "is_deprecated"
    pub advice_type: String,
    /// The value of the advice e.g. "true"
    pub value: Value,
    /// The message of the advice e.g. "This attribute is deprecated"
    pub message: String,
    /// The level of the advice e.g. "violation"
    pub advice_level: AdviceLevel,
}
