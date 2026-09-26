// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define metrics going forward.

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    deprecated::Deprecated,
    v2::{
        attribute::AttributeOrGroupRef, entity_association::EntityAssociation, signal_id::SignalId,
        signal_requirement_level::SignalRequirementLevel, stability::Stability, CommonFields,
    },
    YamlValue,
};

/// The type of the metric.
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord, Copy,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum InstrumentSpec {
    /// An up-down counter metric.
    #[serde(rename = "updowncounter")]
    UpDownCounter,
    /// A counter metric.
    Counter,
    /// A gauge metric.
    Gauge,
    /// A histogram metric.
    Histogram,
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
    ///
    /// The list is an implicit `one_of` (telemetry must satisfy at least one entry); each entry is an
    /// entity reference or a nested `one_of`/`all_of` expression.
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
    ///
    /// The list is an implicit `one_of` (telemetry must satisfy at least one entry); each entry is an
    /// entity reference or a nested `one_of`/`all_of` expression.
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
        assert_eq!(metric.instrument, InstrumentSpec::Histogram);
        assert_eq!(metric.unit, "s");
    }

    #[test]
    fn test_instrument_spec_display() {
        assert_eq!(InstrumentSpec::Counter.to_string(), "counter");
        assert_eq!(InstrumentSpec::Gauge.to_string(), "gauge");
        assert_eq!(InstrumentSpec::Histogram.to_string(), "histogram");
        assert_eq!(InstrumentSpec::UpDownCounter.to_string(), "updowncounter");
    }
}
