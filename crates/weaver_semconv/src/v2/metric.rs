// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define metrics going forward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    group::{GroupSpec, InstrumentSpec},
    v2::{attribute::AttributeRef, CommonFields},
};

/// A MetricGroup defines a new metric.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MetricGroup {
    /// The name of the metric.
    pub name: String,
    /// The instrument type that should be used to record the metric. Note that
    /// the semantic conventions must be written using the names of the
    /// synchronous instrument types (counter, gauge, updowncounter and
    /// histogram).
    /// For more details: [Metrics semantic conventions - Instrument types](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-types).
    /// Note: This field is required if type is metric.
    pub instrument: InstrumentSpec,
    /// The unit in which the metric is measured, which should adhere to the
    /// [guidelines](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-units).
    pub unit: String,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeRef>,
    /// Which resources this metric should be associated with.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<String>,
    /// Common fields (like brief, note, attributes).
    #[serde(flatten)]
    pub common: CommonFields,
}

impl MetricGroup {
    /// Converts a v2 span gorup into a v1 GroupSpec.
    pub fn into_v1_group(self) -> GroupSpec {
        GroupSpec {
            id: format!("metric.{}", &self.name),
            r#type: crate::group::GroupType::Metric,
            brief: self.common.brief,
            note: self.common.note,
            prefix: Default::default(),
            extends: None,
            stability: Some(self.common.stability),
            deprecated: self.common.deprecated,
            attributes: self
                .attributes
                .into_iter()
                .map(|a| a.into_v1_attribute())
                .collect(),
            span_kind: None,
            events: Default::default(),
            metric_name: Some(self.name),
            instrument: Some(self.instrument),
            unit: Some(self.unit),
            name: None,
            display_name: None,
            body: None,
            annotations: self.common.annotations,
            entity_associations: self.entity_associations,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_translate(v2: &str, v1: &str) {
        let metric = serde_yaml::from_str::<MetricGroup>(v2).expect("Failed to parse YAML string");
        let expected =
            serde_yaml::from_str::<GroupSpec>(v1).expect("Failed to parse expected YAML");
        assert_eq!(expected, metric.into_v1_group());
    }

    #[test]
    fn test_value_spec_display() {
        parse_and_translate(
            // V2 - Metric
            r#"name: my_metric
brief: Test metric
stability: stable
instrument: histogram
unit: s
"#,
            // V1 - Group
            r#"id: metric.my_metric
type: metric
metric_name: my_metric
brief: Test metric
stability: stable
instrument: histogram
unit: s
"#,
        );
    }
}
