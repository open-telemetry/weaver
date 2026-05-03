// SPDX-License-Identifier: Apache-2.0

//! Configuration for the `registry check` command.

use schemars::JsonSchema;
use serde::Deserialize;

/// Check-specific configuration.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(default)]
#[schemars(inline)]
pub struct CheckConfig {}
