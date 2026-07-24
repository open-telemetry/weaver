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
    /// The method of aggregation to be used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<AggregationMethodSpec>,
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

/// The Aggregation Method
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AggregationMethodSpec {
    /// Use the Default Aggregation Method.
    Default,
    /// Use the Sum Aggregation Method.
    Sum,
    /// Use the Last Value Aggregation Method.
    LastValue,
    /// Use the Histogram Aggregation Method.
    Histogram,
    /// Use the Explicit Histogram Aggregation Method.
    ExplicitHistogram,
    /// Use the Exponential Histogram Aggregation Method.
    ExponentialHistogram,
}