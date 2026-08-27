// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define metrics going forward.

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    deprecated::Deprecated,
    entity_association::EntityAssociation,
    signal_requirement_level::SignalRequirementLevel,
    stability::Stability,
    v2::{
        attribute::AttributeOrGroupRef,
        signal_id::SignalId,
        CommonFields,
    },
    YamlValue,
};

/// The instrument type that should be used to record the metric.
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord, Copy,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum InstrumentSpec {
    /// A counter metric.
    Counter,
    /// A gauge metric.
    Gauge,
    /// A histogram metric.
    Histogram,
    /// An up-down counter metric.
    #[serde(rename = "updowncounter")]
    UpDownCounter,
}

impl Display for InstrumentSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            InstrumentSpec::Counter => write!(f, "counter"),
            InstrumentSpec::Gauge => write!(f, "gauge"),
            InstrumentSpec::Histogram => write!(f, "histogram"),
            InstrumentSpec::UpDownCounter => write!(f, "updowncounter"),
        }
    }
}

/// Defines a new metric.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    /// The name of the metric.
    pub name: SignalId,
    /// The instrument type that should be used to record the metric.
    pub instrument: InstrumentSpec,
    /// The unit in which the metric is measured.
    pub unit: String,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeOrGroupRef>,
    /// Which resources this metric should be associated with.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<EntityAssociation>,
    /// The requirement level of the metric. Defaults to 'recommended' when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<SignalRequirementLevel>,
    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A refinement of an existing metric.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MetricRefinement {
    /// The ID of the refinement.
    pub id: SignalId,
    /// The name of the metric being refined.
    pub r#ref: SignalId,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeOrGroupRef>,
    /// Which resources this metric should be associated with.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<EntityAssociation>,

    /// Refines the brief description of the signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// Refines the more elaborate description of the signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Refines the stability of the signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the signal is deprecated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// Additional annotations for the signal.
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, YamlValue>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_parsing() {
        let yaml = r#"name: my_metric
brief: Test metric
stability: stable
instrument: histogram
unit: s
requirement_level: opt_in
"#;
        let metric = serde_yaml::from_str::<Metric>(yaml).expect("Failed to parse YAML string");
        assert_eq!(metric.name.to_string(), "my_metric");
        assert_eq!(metric.instrument, InstrumentSpec::Histogram);
        assert_eq!(metric.unit, "s");
    }
}
