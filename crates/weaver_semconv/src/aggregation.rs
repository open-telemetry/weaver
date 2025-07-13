// SPDX-License-Identifier: Apache-2.0

//! Metric specification.
//! 
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::YamlValue;

/// An aggregation specification.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AggregationSpec {
    /// The parameters used in the aggregation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, YamlValue>>,    
}

impl AggregationSpec {
    /// Returns the parameters of the aggregation.
    #[must_use]
    pub fn parameters(&self) -> &Option<HashMap<String, YamlValue>> {
        &self.parameters
    }
}