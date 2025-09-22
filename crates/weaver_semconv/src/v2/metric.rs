// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define metrics going forward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    group::{GroupSpec, InstrumentSpec},
    v2::{
        attribute::{split_attributes_and_groups, AttributeOrGroupRef},
        signal_id::SignalId,
        CommonFields,
    },
};

/// Defines a new metric.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    /// The name of the metric.
    pub name: SignalId,
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
    pub attributes: Vec<AttributeOrGroupRef>,
    /// Which resources this metric should be associated with.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<String>,
    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

impl Metric {
    /// Converts a v2 span group into a v1 GroupSpec.
    #[must_use]
    pub fn into_v1_group(self) -> GroupSpec {
        let (attribute_refs, include_groups) = split_attributes_and_groups(self.attributes);
        GroupSpec {
            id: format!("metric.{}", &self.name),
            r#type: crate::group::GroupType::Metric,
            brief: self.common.brief,
            note: self.common.note,
            prefix: Default::default(),
            extends: None,
            include_groups,
            stability: Some(self.common.stability),
            deprecated: self.common.deprecated,
            attributes: attribute_refs,
            span_kind: None,
            events: Default::default(),
            metric_name: Some(self.name.into_v1()),
            instrument: Some(self.instrument),
            unit: Some(self.unit),
            name: None,
            display_name: None,
            body: None,
            annotations: if self.common.annotations.is_empty() {
                None
            } else {
                Some(self.common.annotations)
            },
            entity_associations: self.entity_associations,
            visibility: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_translate(v2: &str, v1: &str) {
        let metric = serde_yaml::from_str::<Metric>(v2).expect("Failed to parse YAML string");
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
