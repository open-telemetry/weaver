// SPDX-License-Identifier: Apache-2.0

//! Specification of a resolved value.

use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};

/// The different types of values.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(tag = "type")]
#[must_use]
pub enum Value {
    /// A integer value.
    Int {
        /// The value
        value: i64,
    },
    /// A double value.
    Double {
        /// The value
        value: OrderedFloat<f64>,
    },
    /// A string value.
    String {
        /// The value
        value: String,
    },
}

impl Value {
    /// Creates a double value from a f64.
    pub fn from_f64(value: f64) -> Self {
        Value::Double {
            value: OrderedFloat(value),
        }
    }
}
