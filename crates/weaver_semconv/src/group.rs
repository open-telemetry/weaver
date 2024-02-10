// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! A group specification.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use validator::{Validate, ValidationError};

use crate::attribute::{AttributeSpec, AttributeTypeSpec, PrimitiveOrArrayTypeSpec};
use crate::group::InstrumentSpec::{Counter, Gauge, Histogram, UpDownCounter};
use crate::stability::StabilitySpec;

/// Group Spec contain the list of semantic conventions and it is the root node
/// of each yaml file.
#[derive(Serialize, Deserialize, Debug, Validate, Clone)]
#[serde(deny_unknown_fields)]
#[validate(schema(function = "validate_group"))]
pub struct GroupSpec {
    /// The id that uniquely identifies the semantic convention.
    pub id: String,
    /// The type of the semantic convention (default to span).
    #[serde(default)]
    pub r#type: ConvTypeSpec,
    /// A brief description of the semantic convention.
    pub brief: String,
    /// A more elaborate description of the semantic convention.
    /// It defaults to an empty string.
    #[serde(default)]
    pub note: String,
    /// Prefix for the attributes for this semantic convention.
    /// It defaults to an empty string.
    #[serde(default)]
    pub prefix: String,
    /// Reference another semantic convention id. It inherits the prefix,
    /// constraints, and all attributes defined in the specified semantic
    /// convention.
    pub extends: Option<String>,
    /// Specifies the stability of the semantic convention.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<StabilitySpec>,
    /// Specifies if the semantic convention is deprecated. The string
    /// provided as <description> MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    pub attributes: Vec<AttributeSpec>,
    /// Additional constraints.
    /// Allow to define additional requirements on the semantic convention.
    /// It defaults to an empty list.
    #[serde(default)]
    pub constraints: Vec<ConstraintSpec>,
    /// Specifies the kind of the span.
    /// Note: only valid if type is span (the default)
    pub span_kind: Option<SpanKindSpec>,
    /// List of strings that specify the ids of event semantic conventions
    /// associated with this span semantic convention.
    /// Note: only valid if type is span (the default)
    #[serde(default)]
    pub events: Vec<String>,
    /// The metric name as described by the [OpenTelemetry Specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/data-model.md#timeseries-model).
    /// Note: This field is required if type is metric.
    pub metric_name: Option<String>,
    /// The instrument type that should be used to record the metric. Note that
    /// the semantic conventions must be written using the names of the
    /// synchronous instrument types (counter, gauge, updowncounter and
    /// histogram).
    /// For more details: [Metrics semantic conventions - Instrument types](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-types).
    /// Note: This field is required if type is metric.
    pub instrument: Option<InstrumentSpec>,
    /// The unit in which the metric is measured, which should adhere to the
    /// [guidelines](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-units).
    /// Note: This field is required if type is metric.
    pub unit: Option<String>,
    /// The name of the event. If not specified, the prefix is used.
    /// If prefix is empty (or unspecified), name is required.
    pub name: Option<String>,
}

/// Validation logic for the group.
fn validate_group(group: &GroupSpec) -> Result<(), ValidationError> {
    // If deprecated is present and stability differs from deprecated, this
    // will result in an error.
    if group.deprecated.is_some()
        && group.stability.is_some()
        && group.stability != Some(StabilitySpec::Deprecated)
    {
        return Err(ValidationError::new(
            "This group contains a deprecated field but the stability is not set to deprecated.",
        ));
    }

    // Fields span_kind and events are only valid if type is span (the default).
    if group.r#type != ConvTypeSpec::Span {
        if group.span_kind.is_some() {
            return Err(ValidationError::new(
                "This group contains a span_kind field but the type is not set to span.",
            ));
        }
        if !group.events.is_empty() {
            return Err(ValidationError::new(
                "This group contains an events field but the type is not set to span.",
            ));
        }
    }

    // Field name is required if prefix is empty and if type is event.
    if group.r#type == ConvTypeSpec::Event && group.prefix.is_empty() && group.name.is_none() {
        return Err(ValidationError::new(
            "This group contains an event type but the prefix is empty and the name is not set.",
        ));
    }

    // Fields metric_name, instrument and unit are required if type is metric.
    if group.r#type == ConvTypeSpec::Metric {
        if group.metric_name.is_none() {
            return Err(ValidationError::new(
                "This group contains a metric type but the metric_name is not set.",
            ));
        }
        if group.instrument.is_none() {
            return Err(ValidationError::new(
                "This group contains a metric type but the instrument is not set.",
            ));
        }
        if group.unit.is_none() {
            return Err(ValidationError::new(
                "This group contains a metric type but the unit is not set.",
            ));
        }
    }

    // Validates the attributes.
    for attribute in &group.attributes {
        // If deprecated is present and stability differs from deprecated, this
        // will result in an error.
        match attribute {
            AttributeSpec::Id {
                stability,
                deprecated,
                ..
            }
            | AttributeSpec::Ref {
                stability,
                deprecated,
                ..
            } => {
                if deprecated.is_some()
                    && stability.is_some()
                    && *stability != Some(StabilitySpec::Deprecated)
                {
                    return Err(ValidationError::new("This attribute contains a deprecated field but the stability is not set to deprecated."));
                }
            }
        }

        // Examples are required only for string and string array attributes.
        if let AttributeSpec::Id {
            r#type, examples, ..
        } = attribute
        {
            if examples.is_some() {
                continue;
            }

            if *r#type == AttributeTypeSpec::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String) {
                return Err(ValidationError::new(
                    "This attribute is a string but it does not contain any examples.",
                ));
            }
            if *r#type == AttributeTypeSpec::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings) {
                return Err(ValidationError::new(
                    "This attribute is a string array but it does not contain any examples.",
                ));
            }
        }
    }

    Ok(())
}

/// The different types of groups (specification).
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ConvTypeSpec {
    /// Attribute group (attribute_group type) defines a set of attributes that
    /// can be declared once and referenced by semantic conventions for
    /// different signals, for example spans and logs. Attribute groups don't
    /// have any specific fields and follow the general semconv semantics.
    AttributeGroup,
    /// Span semantic convention.
    Span,
    /// Event semantic convention.
    Event,
    /// Metric semantic convention.
    Metric,
    /// The metric group semconv is a group where related metric attributes can
    /// be defined and then referenced from other metric groups using ref.
    MetricGroup,
    /// A group of resources.
    Resource,
    /// Scope.
    Scope,
}

impl Default for ConvTypeSpec {
    /// Returns the default convention type that is span based on
    /// the OpenTelemetry specification.
    fn default() -> Self {
        Self::Span
    }
}

/// The span kind.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SpanKindSpec {
    /// An internal span.
    Internal,
    /// A client span.
    Client,
    /// A server span.
    Server,
    /// A producer span.
    Producer,
    /// A consumer span.
    Consumer,
}

/// Allow to define additional requirements on the semantic convention.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ConstraintSpec {
    /// any_of accepts a list of sequences. Each sequence contains a list of
    /// attribute ids that are required. any_of enforces that all attributes
    /// of at least one of the sequences are set.
    #[serde(default)]
    pub any_of: Vec<String>,
    /// include accepts a semantic conventions id. It includes as part of this
    /// semantic convention all constraints and required attributes that are
    /// not already defined in the current semantic convention.
    pub include: Option<String>,
}

/// The type of the metric.
#[derive(Serialize, Deserialize, Debug, Clone)]
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

/// Implements a human readable display for the instrument.
impl Display for InstrumentSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            UpDownCounter => write!(f, "updowncounter"),
            Counter => write!(f, "counter"),
            Gauge => write!(f, "gauge"),
            Histogram => write!(f, "histogram"),
        }
    }
}
