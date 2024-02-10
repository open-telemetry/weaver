// SPDX-License-Identifier: Apache-2.0

//! Log record specification.

use crate::attribute::Attribute;
use crate::tags::Tags;
use serde::{Deserialize, Serialize};

/// A log record specification.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct LogRecord {
    /// The id of the log record.
    pub id: String,
    /// The attributes of the log record.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Attribute>,
    /// Brief description of the log record.
    pub brief: Option<String>,
    /// Longer description.
    /// It defaults to an empty string.
    pub note: Option<String>,
    /// A set of tags for the log record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
}

/// The type of body of a log record.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum BodyType {
    /// A boolean body.
    Boolean(bool),
    /// An integer body.
    Int(i64),
    /// A double body.
    Double(f64),
    /// A string body.
    String(String),
    /// A boolean array body.
    #[serde(rename = "boolean[]")]
    Booleans(Vec<String>),
    /// An integer array body.
    #[serde(rename = "int[]")]
    Ints(Vec<String>),
    /// A double array body.
    #[serde(rename = "double[]")]
    Doubles(Vec<String>),
    /// A string array body.
    #[serde(rename = "string[]")]
    Strings(Vec<String>),
}
