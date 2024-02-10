// SPDX-License-Identifier: Apache-2.0

//! Instrumentation library specification.

use crate::tags::Tags;
use serde::{Deserialize, Serialize};

/// An instrumentation library specification.
/// MUST be used both by applications and libraries.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct InstrumentationLibrary {
    /// An optional name for the instrumentation library.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// An optional version for the instrumentation library.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// A set of tags for the instrumentation library.
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Tags>,
}
