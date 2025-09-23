// SPDX-License-Identifier: Apache-2.0

//! Definition of a policy violation.

use schemars::JsonSchema;
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
                    "id={id}, category={category}, group={group}, attr={attr}"
                )
            }
            Violation::Advice(Advice {
                advice_type: r#type,
                value,
                message,
                advice_level,
                signal_type,
                signal_name,
            }) => {
                write!(
                    f,
                    "type={type}, value={value}, message={message}, advice_level={advice_level:?}, signal_type={signal_type:?}, signal_name={signal_name:?}"
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
#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, PartialOrd, Ord, Eq, Hash, JsonSchema,
)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Advice {
    /// The type of advice e.g. "is_deprecated". This should be a short,
    /// machine-readable string that categorizes the advice.
    pub advice_type: String,

    /// The context associated with the advice e.g. { "attribute_name": "foo.bar" }
    /// Context values may be used with custom templates and filters to query, summarize, and format advice.
    pub value: Value,

    /// The human-readable message of the advice e.g. "This attribute 'foo.bar' is deprecated, reason: 'use foo.baz'"
    /// The message, along with signal_name and signal_type, should contain enough information to understand the advice and
    /// identify the issue and how to fix it.
    /// Some of the values used in the message may be also present in the `value` field to support report customization.
    pub message: String,

    /// The level of the advice e.g. "violation"
    pub advice_level: AdviceLevel,

    /// The signal type the advice applies to: "span", "metric", "entity", "log" (aka "event"), or "profile"
    pub signal_type: Option<String>,

    /// The signal name the advice applies to e.g. "http.server.request.duration".
    pub signal_name: Option<String>,
}
