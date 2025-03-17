// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! A group specification.

use schemars::JsonSchema;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::any_value::AnyValueSpec;
use crate::attribute::{AttributeSpec, AttributeType, PrimitiveOrArrayTypeSpec};
use crate::deprecated::Deprecated;
use crate::group::InstrumentSpec::{Counter, Gauge, Histogram, UpDownCounter};
use crate::stability::Stability;
use crate::{Error, YamlValue};
use weaver_common::result::WResult;

/// Group Spec contain the list of semantic conventions for attributes,
/// metrics, events, spans, etc.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct GroupSpec {
    /// The id that uniquely identifies the semantic convention.
    pub id: String,
    /// The type of the semantic convention.
    #[serde(default)]
    pub r#type: GroupType,
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
    /// Reference another semantic convention id. It inherits all
    /// attributes defined in the specified semantic
    /// convention.
    pub extends: Option<String>,
    /// Specifies the stability of the semantic convention.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the semantic convention is deprecated. The string
    /// provided as <description> MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(
        deserialize_with = "crate::deprecated::deserialize_option_deprecated",
        default
    )]
    pub deprecated: Option<Deprecated>,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    pub attributes: Vec<AttributeSpec>,
    /// List of constraints
    pub constraints: Option<Vec<serde_yaml::Value>>,
    /// Specifies the kind of the span.
    /// Note: only valid if type is span
    pub span_kind: Option<SpanKindSpec>,
    /// List of strings that specify the ids of event semantic conventions
    /// associated with this span semantic convention.
    /// Note: only valid if type is span
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
    /// The readable name for attribute groups used when generating registry tables.
    pub display_name: Option<String>,
    /// The event body definition
    /// Note: only valid if type is event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<AnyValueSpec>,
    /// Annotations for the group.
    pub annotations: Option<HashMap<String, YamlValue>>,
}

impl GroupSpec {
    /// Validation logic for the group.
    pub(crate) fn validate(&self, path_or_url: &str) -> WResult<(), Error> {
        let mut errors = vec![];

        if !self.prefix.is_empty() {
            errors.push(Error::InvalidGroupUsesPrefix {
                path_or_url: path_or_url.to_owned(),
                group_id: self.id.clone(),
            });
        }

        // Field stability is required for all group types except attribute group.
        if self.r#type != GroupType::AttributeGroup && self.stability.is_none() {
            errors.push(Error::InvalidGroupStability {
                path_or_url: path_or_url.to_owned(),
                group_id: self.id.clone(),
                error: "This group does not contain a stability field.".to_owned(),
            });
        }

        // `deprecated` stability is deprecated
        if self.stability == Some(Stability::Deprecated) {
            errors.push(Error::InvalidGroupStability {
                path_or_url: path_or_url.to_owned(),
                group_id: self.id.clone(),
                error: "Group stability is set to 'deprecated' which is no longer supported."
                    .to_owned(),
            });
        }

        // Groups should only reference attributes once.
        validate_duplicate_attribute_ref(&mut errors, &self.attributes, &self.id, path_or_url);

        // All types, except metric and event, must have extends or attributes or both.
        if self.r#type != GroupType::Metric
            && self.r#type != GroupType::Event
            && self.extends.is_none()
            && self.attributes.is_empty()
        {
            errors.push(Error::InvalidGroupMissingExtendsOrAttributes {
                path_or_url: path_or_url.to_owned(),
                group_id: self.id.clone(),
                error: "This group does not contain an extends or attributes field.".to_owned(),
            });
        }

        // Fields span_kind and events are only valid if type is span.
        if self.r#type != GroupType::Span && self.r#type != GroupType::Undefined {
            if self.span_kind.is_some() {
                errors.push(Error::InvalidGroup {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains a span_kind field but the type is not set to span."
                        .to_owned(),
                });
            }
            if !self.events.is_empty() {
                errors.push(Error::InvalidGroup {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains an events field but the type is not set to span."
                        .to_owned(),
                });
            }
        }

        // Group type is required.
        if self.r#type == GroupType::Undefined {
            errors.push(Error::InvalidGroupMissingType {
                path_or_url: path_or_url.to_owned(),
                group_id: self.id.clone(),
                error: "This group does not contain a type field.".to_owned(),
            });
        }

        // Span kind is required if type is span.
        if self.r#type == GroupType::Span && self.span_kind.is_none() {
            errors.push(Error::InvalidSpanMissingSpanKind {
                path_or_url: path_or_url.to_owned(),
                group_id: self.id.clone(),
                error: "This group is a Span but the span_kind is not set.".to_owned(),
            });
        }

        // Field name is required if prefix is empty and if type is event.
        if self.r#type == GroupType::Event {
            if self.body.is_some() && self.name.is_none() {
                // Must have a name which is assigned to event.name for log based events
                errors.push(Error::InvalidGroup {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains an event type with a body definition but the name is not set.".to_owned(),
                });
            }
            if self.body.is_none() && self.name.is_none() && self.prefix.is_empty() {
                // This is ONLY for backward compatibility of span based events.
                // Must have a name (whether explicit or via a prefix which will derive the name)
                errors.push(Error::InvalidGroup {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains an event type but the name is not set and no prefix is defined.".to_owned(),
                });
            }

            validate_any_value(&mut errors, self.body.as_ref(), &self.id, path_or_url);

            match validate_any_value_examples(
                &mut errors,
                self.body.as_ref(),
                &self.id,
                path_or_url,
            ) {
                WResult::Ok(_) => {}
                WResult::OkWithNFEs(_, errs) => errors.extend(errs),
                WResult::FatalErr(err) => return WResult::FatalErr(err),
            }
        } else if self.body.is_some() {
            // Make sure that body is only used for events
            errors.push(Error::InvalidGroup {
                path_or_url: path_or_url.to_owned(),
                group_id: self.id.clone(),
                error: "This group contains a body field but the type is not set to event."
                    .to_owned(),
            });
        }

        // Fields metric_name, instrument and unit are required if type is metric.
        if self.r#type == GroupType::Metric {
            if self.metric_name.is_none() {
                errors.push(Error::InvalidMetric {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains a metric type but the metric_name is not set."
                        .to_owned(),
                });
            }
            if self.instrument.is_none() {
                errors.push(Error::InvalidMetric {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains a metric type but the instrument is not set."
                        .to_owned(),
                });
            }
            if self.unit.is_none() {
                errors.push(Error::InvalidMetric {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains a metric type but the unit is not set.".to_owned(),
                });
            }
        }

        // Validates the attributes.
        for attribute in &self.attributes {
            match attribute {
                AttributeSpec::Id {
                    brief,
                    deprecated,
                    stability,
                    r#type,
                    ..
                } => {
                    if brief.is_none() && deprecated.is_none() {
                        errors.push(Error::InvalidAttribute {
                            path_or_url: path_or_url.to_owned(),
                            group_id: self.id.clone(),
                            attribute_id: attribute.id(),
                            error: "This attribute is not deprecated and does not contain a brief field.".to_owned(),
                        });
                    }

                    if stability.is_none() {
                        errors.push(Error::InvalidAttributeWarning {
                            path_or_url: path_or_url.to_owned(),
                            group_id: self.id.clone(),
                            attribute_id: attribute.id(),
                            error: "Missing stability field.".to_owned(),
                        });
                    } else if stability.clone() == Some(Stability::Deprecated) {
                        errors.push(Error::InvalidAttributeWarning {
                            path_or_url: path_or_url.to_owned(),
                            group_id: self.id.clone(),
                            attribute_id: attribute.id(),
                            error: "Attribute stability is set to 'deprecated' which is no longer supported.".to_owned(),
                        });
                    }

                    if let AttributeType::Enum { members, .. } = r#type {
                        for member in members {
                            if member.stability.is_none() {
                                errors.push(Error::InvalidAttributeWarning {
                                    path_or_url: path_or_url.to_owned(),
                                    group_id: self.id.clone(),
                                    attribute_id: attribute.id(),
                                    error: format!(
                                        "Missing stability field on enum member {}.",
                                        member.id
                                    ),
                                });
                            } else if member.stability == Some(Stability::Deprecated) {
                                errors.push(Error::InvalidAttributeWarning {
                                    path_or_url: path_or_url.to_owned(),
                                    group_id: self.id.clone(),
                                    attribute_id: attribute.id(),
                                    error: format!(
                                        "Member {} stability is set to 'deprecated' which is no longer supported.",
                                        member.id
                                    ),
                                });
                            }
                        }
                    }
                }
                AttributeSpec::Ref { .. } => {}
            }

            // Examples are required only for string and string array attributes.
            // When examples are set, the attribute type and examples type must match.
            if let AttributeSpec::Id {
                id,
                r#type,
                examples,
                ..
            } = attribute
            {
                if let Some(examples) = examples {
                    match examples.validate(r#type, &self.id, id, path_or_url) {
                        WResult::Ok(_) => {}
                        WResult::OkWithNFEs(_, errs) => errors.extend(errs),
                        WResult::FatalErr(err) => return WResult::FatalErr(err),
                    }
                } else {
                    // No examples are set.

                    // string attributes must have examples.
                    if *r#type == AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String)
                    {
                        errors.push(Error::InvalidExampleWarning {
                            path_or_url: path_or_url.to_owned(),
                            group_id: self.id.clone(),
                            attribute_id: attribute.id(),
                            error:
                                "This attribute is a string but it does not contain any examples."
                                    .to_owned(),
                        });
                    }

                    // string array attributes must have examples.
                    if *r#type == AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings)
                    {
                        errors.push(Error::InvalidExampleWarning {
                            path_or_url: path_or_url.to_owned(),
                            group_id: self.id.clone(),
                            attribute_id: attribute.id(),
                            error:
                            "This attribute is a string array but it does not contain any examples."
                                .to_owned(),
                        });
                    }
                }
            }

            // Produce a warning if `allow_custom_values` is Some.
            if let AttributeSpec::Id {
                r#type:
                    AttributeType::Enum {
                        allow_custom_values: Some(_),
                        ..
                    },
                ..
            } = attribute
            {
                errors.push(Error::InvalidAttributeAllowCustomValues {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    attribute_id: attribute.id(),
                    error: "This attribute is an enum using allow_custom_values. This is no longer used.".to_owned(),
                });
            }
        }

        WResult::with_non_fatal_errors((), errors)
    }
}

fn validate_duplicate_attribute_ref(
    errors: &mut Vec<Error>,
    attributes: &[AttributeSpec],
    group_id: &str,
    path_or_url: &str,
) {
    let mut seen = HashSet::new();
    for a in attributes.iter() {
        if let AttributeSpec::Ref { r#ref, .. } = a {
            if !seen.insert(r#ref.to_owned()) {
                errors.push(Error::InvalidGroupDuplicateAttributeRef {
                    path_or_url: path_or_url.to_owned(),
                    group_id: group_id.to_owned(),
                    attribute_ref: r#ref.to_owned(),
                });
            }
        }
    }
}

fn validate_any_value_examples(
    errors: &mut Vec<Error>,
    any_value: Option<&AnyValueSpec>,
    group_id: &str,
    path_or_url: &str,
) -> WResult<(), Error> {
    if let Some(value) = any_value {
        if let Some(examples) = &value.common().examples {
            match examples.validate_any_value(value, group_id, path_or_url) {
                WResult::Ok(_) => {}
                WResult::OkWithNFEs(_, errs) => errors.extend(errs),
                WResult::FatalErr(err) => return WResult::FatalErr(err),
            }
        } else {
            match value {
                AnyValueSpec::String { .. } | AnyValueSpec::Strings { .. } => {
                    errors.push(Error::InvalidAnyValueExampleError {
                        path_or_url: path_or_url.to_owned(),
                        group_id: group_id.to_owned(),
                        value_id: value.id(),
                        error: format!(
                            "This value is a {} but it does not contain any examples.",
                            if let AnyValueSpec::String { .. } = value {
                                "string"
                            } else {
                                "string array"
                            }
                        ),
                    });
                }
                _ => {}
            }
        }

        if let AnyValueSpec::Map { fields, .. } = value {
            for field in fields {
                if let WResult::FatalErr(err) =
                    validate_any_value_examples(errors, Some(field), group_id, path_or_url)
                {
                    return WResult::FatalErr(err);
                }
            }
        }
    }

    WResult::Ok(())
}

fn validate_any_value(
    errors: &mut Vec<Error>,
    any_value: Option<&AnyValueSpec>,
    group_id: &str,
    path_or_url: &str,
) {
    if let Some(value) = any_value {
        if value.common().stability.is_none() {
            errors.push(Error::InvalidAnyValue {
                path_or_url: path_or_url.to_owned(),
                group_id: group_id.to_owned(),
                value_id: value.id(),
                error: "Missing stability field.".to_owned(),
            });
        }

        match value {
            AnyValueSpec::Enum { members, .. } => {
                for member in members {
                    if member.stability.is_none() {
                        errors.push(Error::InvalidAnyValue {
                            path_or_url: path_or_url.to_owned(),
                            group_id: group_id.to_owned(),
                            value_id: value.id(),
                            error: format!(
                                "Missing stability field for enum member {}.",
                                member.id
                            ),
                        });
                    }
                }
            }
            AnyValueSpec::Map { fields, .. } => {
                for field in fields {
                    validate_any_value(errors, Some(field), group_id, path_or_url);
                }
            }
            _ => {}
        };
    }
}

/// The different types of groups (specification).
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Hash, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupType {
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
    /// Undefined group type.
    Undefined,
}

impl Default for GroupType {
    /// Returns the default convention type.
    /// The Undefined type is used to indicate that the type is not set.
    /// This is used for validation purposes.
    fn default() -> Self {
        Self::Undefined
    }
}

/// The span kind.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
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

/// The type of the metric.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
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

#[cfg(test)]
mod tests {
    use crate::any_value::AnyValueCommonSpec;
    use crate::attribute::{
        BasicRequirementLevelSpec, EnumEntriesSpec, Examples, RequirementLevel, ValueSpec,
    };
    use crate::deprecated::Deprecated;
    use crate::Error::{
        CompoundError, InvalidAttributeAllowCustomValues, InvalidAttributeWarning,
        InvalidExampleWarning, InvalidGroup, InvalidGroupMissingExtendsOrAttributes,
        InvalidGroupMissingType, InvalidGroupStability, InvalidGroupUsesPrefix, InvalidMetric,
        InvalidSpanMissingSpanKind,
    };

    use super::*;

    #[test]
    fn test_validate_group() {
        let mut group = GroupSpec {
            id: "test".to_owned(),
            r#type: GroupType::Span,
            brief: "test".to_owned(),
            note: "test".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: Some(Stability::Development),
            constraints: None,
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            attributes: vec![AttributeSpec::Id {
                id: "test".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: None,
                stability: Some(Stability::Development),
                deprecated: Some(Deprecated::Obsoleted {
                    note: "".to_owned(),
                }),
                examples: Some(Examples::String("test".to_owned())),
                tag: None,
                requirement_level: Default::default(),
                sampling_relevant: None,
                note: "".to_owned(),
                annotations: None,
            }],
            span_kind: Some(SpanKindSpec::Client),
            events: vec!["event".to_owned()],
            metric_name: None,
            instrument: None,
            unit: None,
            name: None,
            display_name: None,
            body: None,
            annotations: None,
        };
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        // group has a prefix.
        group.prefix = "test".to_owned();
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupUsesPrefix {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned()
            }),
            result
        );

        // Span kind is missing on a span group.
        group.prefix = "".to_owned();
        group.span_kind = None;
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidSpanMissingSpanKind {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "This group is a Span but the span_kind is not set.".to_owned(),
            },),
            result
        );

        // Group type is missing on a group.
        group.r#type = GroupType::Undefined;
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupMissingType {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "This group does not contain a type field.".to_owned(),
            }),
            result
        );

        // Span kind is set but the type is not span.
        group.span_kind = Some(SpanKindSpec::Client);
        group.r#type = GroupType::Metric;
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(CompoundError(vec![
                InvalidGroup {
                    path_or_url: "<test>".to_owned(),
                    group_id: "test".to_owned(),
                    error: "This group contains a span_kind field but the type is not set to span."
                        .to_owned(),
                },
                InvalidGroup {
                    path_or_url: "<test>".to_owned(),
                    group_id: "test".to_owned(),
                    error: "This group contains an events field but the type is not set to span."
                        .to_owned(),
                },
                InvalidMetric {
                    path_or_url: "<test>".to_owned(),
                    group_id: "test".to_owned(),
                    error: "This group contains a metric type but the metric_name is not set."
                        .to_owned(),
                },
                InvalidMetric {
                    path_or_url: "<test>".to_owned(),
                    group_id: "test".to_owned(),
                    error: "This group contains a metric type but the instrument is not set."
                        .to_owned(),
                },
                InvalidMetric {
                    path_or_url: "<test>".to_owned(),
                    group_id: "test".to_owned(),
                    error: "This group contains a metric type but the unit is not set.".to_owned(),
                },
            ],),),
            result
        );

        // Field name is required if prefix is empty and if type is event.
        group.r#type = GroupType::Event;
        "".clone_into(&mut group.prefix);
        group.name = None;
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(Err(
            CompoundError(
                vec![
                    InvalidGroup {
                        path_or_url: "<test>".to_owned(),
                        group_id: "test".to_owned(),
                        error: "This group contains a span_kind field but the type is not set to span.".to_owned(),
                    },
                    InvalidGroup {
                        path_or_url: "<test>".to_owned(),
                        group_id: "test".to_owned(),
                        error: "This group contains an events field but the type is not set to span.".to_owned(),
                    },
                    InvalidGroup {
                        path_or_url: "<test>".to_owned(),
                        group_id: "test".to_owned(),
                        error: "This group contains an event type but the name is not set and no prefix is defined.".to_owned(),
                    },
                ],
            ),
        ), result);
    }

    #[test]
    fn test_validate_attribute() {
        let mut group = GroupSpec {
            id: "test".to_owned(),
            r#type: GroupType::Span,
            brief: "test".to_owned(),
            note: "test".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: Some(Stability::Development),
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            attributes: vec![AttributeSpec::Id {
                id: "test".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: None,
                stability: Some(Stability::Development),
                deprecated: Some(Deprecated::Obsoleted {
                    note: "".to_owned(),
                }),
                examples: Some(Examples::String("test".to_owned())),
                tag: None,
                requirement_level: Default::default(),
                sampling_relevant: None,
                note: "".to_owned(),
                annotations: None,
            }],
            span_kind: Some(SpanKindSpec::Client),
            events: vec!["event".to_owned()],
            metric_name: None,
            instrument: None,
            unit: None,
            name: None,
            display_name: None,
            body: None,
            annotations: None,
            constraints: None,
        };
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        // Examples are mandatory for string attributes.
        group.attributes = vec![AttributeSpec::Id {
            id: "test".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: None,
            stability: Some(Stability::Development),
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            examples: None,
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: "".to_owned(),
            annotations: None,
        }];
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidExampleWarning {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                attribute_id: "test".to_owned(),
                error: "This attribute is a string but it does not contain any examples."
                    .to_owned(),
            },),
            result
        );

        // Examples are mandatory for strings attributes.
        group.attributes = vec![AttributeSpec::Id {
            id: "test".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings),
            brief: None,
            stability: Some(Stability::Development),
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            examples: None,
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: "".to_owned(),
            annotations: None,
        }];
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidExampleWarning {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                attribute_id: "test".to_owned(),
                error: "This attribute is a string array but it does not contain any examples."
                    .to_owned(),
            },),
            result
        );

        // Stability is missing.
        group.attributes = vec![AttributeSpec::Id {
            id: "test".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: None,
            stability: None,
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            examples: Some(Examples::String("test".to_owned())),
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: "".to_owned(),
            annotations: None,
        }];
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidAttributeWarning {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                attribute_id: "test".to_owned(),
                error: "Missing stability field.".to_owned(),
            },),
            result
        );

        // Stability is set to deprecated.
        group.attributes = vec![AttributeSpec::Id {
            id: "test".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: None,
            stability: Some(Stability::Deprecated),
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            examples: Some(Examples::String("test".to_owned())),
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: "".to_owned(),
            annotations: None,
        }];
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidAttributeWarning {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                attribute_id: "test".to_owned(),
                error: "Attribute stability is set to 'deprecated' which is no longer supported."
                    .to_owned(),
            },),
            result
        );

        // Stability is missing on enum member.
        group.attributes = vec![AttributeSpec::Id {
            id: "test".to_owned(),
            r#type: AttributeType::Enum {
                allow_custom_values: None,
                members: vec![EnumEntriesSpec {
                    id: "member_id".to_owned(),
                    value: ValueSpec::String("member_value".to_owned()),
                    brief: None,
                    note: None,
                    stability: None,
                    deprecated: None,
                }],
            },
            brief: None,
            stability: Some(Stability::Stable),
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            examples: Some(Examples::String("test".to_owned())),
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: "".to_owned(),
            annotations: None,
        }];
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidAttributeWarning {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                attribute_id: "test".to_owned(),
                error: "Missing stability field on enum member member_id.".to_owned(),
            },),
            result
        );

        // Stability is set to deprecated on enum member.
        group.attributes = vec![AttributeSpec::Id {
            id: "test".to_owned(),
            r#type: AttributeType::Enum {
                allow_custom_values: None,
                members: vec![EnumEntriesSpec {
                    id: "member_id".to_owned(),
                    value: ValueSpec::String("member_value".to_owned()),
                    brief: None,
                    note: None,
                    stability: Some(Stability::Deprecated),
                    deprecated: None,
                }],
            },
            brief: None,
            stability: Some(Stability::Stable),
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            examples: Some(Examples::String("test".to_owned())),
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: "".to_owned(),
            annotations: None,
        }];
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidAttributeWarning {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                attribute_id: "test".to_owned(),
                error: "Member member_id stability is set to 'deprecated' which is no longer supported.".to_owned(),
            },),
            result
        );
    }

    #[test]
    fn test_allow_custom_values() {
        let mut group = GroupSpec {
            id: "test".to_owned(),
            r#type: GroupType::Span,
            brief: "test".to_owned(),
            note: "test".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: Some(Stability::Development),
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            attributes: vec![AttributeSpec::Id {
                id: "test".to_owned(),
                r#type: AttributeType::Enum {
                    members: vec![],
                    allow_custom_values: Some(true),
                },
                brief: None,
                stability: Some(Stability::Development),
                deprecated: Some(Deprecated::Obsoleted {
                    note: "".to_owned(),
                }),
                examples: Some(Examples::String("test".to_owned())),
                tag: None,
                requirement_level: Default::default(),
                sampling_relevant: None,
                note: "".to_owned(),
                annotations: None,
            }],
            span_kind: Some(SpanKindSpec::Client),
            events: vec!["event".to_owned()],
            metric_name: None,
            instrument: None,
            unit: None,
            name: None,
            display_name: None,
            body: None,
            annotations: None,
            constraints: None,
        };
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidAttributeAllowCustomValues {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                attribute_id: "test".to_owned(),
                error:
                    "This attribute is an enum using allow_custom_values. This is no longer used."
                        .to_owned(),
            },),
            result
        );
        // Test that allow_custom_values is not set.
        group.attributes = vec![AttributeSpec::Id {
            id: "test".to_owned(),
            r#type: AttributeType::Enum {
                members: vec![],
                allow_custom_values: None,
            },
            brief: None,
            stability: Some(Stability::Development),
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            examples: Some(Examples::String("test".to_owned())),
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: "".to_owned(),
            annotations: None,
        }];
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_event() {
        let mut group = GroupSpec {
            id: "test".to_owned(),
            r#type: GroupType::Event,
            name: Some("test_event".to_owned()),
            brief: "test".to_owned(),
            note: "test".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: Some(Stability::Development),
            constraints: None,
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            span_kind: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            display_name: None,
            attributes: vec![],
            body: Some(AnyValueSpec::String {
                common: AnyValueCommonSpec {
                    id: "id".to_owned(),
                    brief: "brief".to_owned(),
                    note: "note".to_owned(),
                    stability: Some(Stability::Stable),
                    examples: Some(Examples::String("test".to_owned())),
                    requirement_level: RequirementLevel::Basic(
                        BasicRequirementLevelSpec::Recommended,
                    ),
                },
            }),
            annotations: None,
        };
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        // Examples are mandatory for string attributes.
        group.body = Some(AnyValueSpec::String {
            common: AnyValueCommonSpec {
                id: "string_id".to_owned(),
                brief: "brief".to_owned(),
                note: "note".to_owned(),
                stability: Some(Stability::Stable),
                examples: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            },
        });

        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(Error::InvalidAnyValueExampleError {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                value_id: "string_id".to_owned(),
                error: "This value is a string but it does not contain any examples.".to_owned(),
            },),
            result
        );

        // Examples are mandatory for strings attributes.
        group.body = Some(AnyValueSpec::Strings {
            common: AnyValueCommonSpec {
                id: "string_array_id".to_owned(),
                brief: "brief".to_owned(),
                note: "note".to_owned(),
                stability: Some(Stability::Stable),
                examples: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            },
        });
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(Error::InvalidAnyValueExampleError {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                value_id: "string_array_id".to_owned(),
                error: "This value is a string array but it does not contain any examples."
                    .to_owned(),
            },),
            result
        );

        // Examples are not required for Map.
        group.body = Some(AnyValueSpec::Map {
            common: AnyValueCommonSpec {
                id: "map_id".to_owned(),
                brief: "brief".to_owned(),
                note: "note".to_owned(),
                stability: Some(Stability::Stable),
                examples: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            },
            fields: vec![AnyValueSpec::String {
                common: AnyValueCommonSpec {
                    id: "string_id".to_owned(),
                    brief: "brief".to_owned(),
                    note: "note".to_owned(),
                    stability: Some(Stability::Stable),
                    examples: Some(Examples::String("test".to_owned())),
                    requirement_level: RequirementLevel::Basic(
                        BasicRequirementLevelSpec::Recommended,
                    ),
                },
            }],
        });

        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        // Examples are not required for Map[].
        group.body = Some(AnyValueSpec::Maps {
            common: AnyValueCommonSpec {
                id: "map_id".to_owned(),
                brief: "brief".to_owned(),
                note: "note".to_owned(),
                stability: Some(Stability::Stable),
                examples: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            },
            fields: vec![AnyValueSpec::String {
                common: AnyValueCommonSpec {
                    id: "string_id".to_owned(),
                    brief: "brief".to_owned(),
                    note: "note".to_owned(),
                    stability: Some(Stability::Stable),
                    examples: Some(Examples::String("test".to_owned())),
                    requirement_level: RequirementLevel::Basic(
                        BasicRequirementLevelSpec::Recommended,
                    ),
                },
            }],
        });

        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        // Examples are mandatory for string attributes even if nested
        group.body = Some(AnyValueSpec::Map {
            common: AnyValueCommonSpec {
                id: "map_id".to_owned(),
                brief: "brief".to_owned(),
                note: "note".to_owned(),
                stability: Some(Stability::Stable),
                examples: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            },
            fields: vec![AnyValueSpec::String {
                common: AnyValueCommonSpec {
                    id: "nested_string_id".to_owned(),
                    brief: "brief".to_owned(),
                    note: "note".to_owned(),
                    stability: Some(Stability::Stable),
                    examples: None,
                    requirement_level: RequirementLevel::Basic(
                        BasicRequirementLevelSpec::Recommended,
                    ),
                },
            }],
        });

        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(Error::InvalidAnyValueExampleError {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                value_id: "nested_string_id".to_owned(),
                error: "This value is a string but it does not contain any examples.".to_owned(),
            },),
            result
        );

        // Examples are mandatory for strings attributes even if nested
        group.body = Some(AnyValueSpec::Map {
            common: AnyValueCommonSpec {
                id: "map_id".to_owned(),
                brief: "brief".to_owned(),
                note: "note".to_owned(),
                stability: Some(Stability::Stable),
                examples: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            },
            fields: vec![AnyValueSpec::Strings {
                common: AnyValueCommonSpec {
                    id: "nested_strings_id".to_owned(),
                    brief: "brief".to_owned(),
                    note: "note".to_owned(),
                    stability: Some(Stability::Stable),
                    examples: None,
                    requirement_level: RequirementLevel::Basic(
                        BasicRequirementLevelSpec::Recommended,
                    ),
                },
            }],
        });

        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(Error::InvalidAnyValueExampleError {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                value_id: "nested_strings_id".to_owned(),
                error: "This value is a string array but it does not contain any examples."
                    .to_owned(),
            },),
            result
        );
    }

    #[test]
    fn test_validate_event_stability() {
        let mut group = GroupSpec {
            id: "test".to_owned(),
            r#type: GroupType::Event,
            name: Some("test_event".to_owned()),
            brief: "test".to_owned(),
            note: "test".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: Some(Stability::Stable),
            deprecated: None,
            span_kind: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            display_name: None,
            attributes: vec![],
            body: Some(AnyValueSpec::String {
                common: AnyValueCommonSpec {
                    id: "id".to_owned(),
                    brief: "brief".to_owned(),
                    note: "note".to_owned(),
                    stability: Some(Stability::Stable),
                    examples: Some(Examples::String("test".to_owned())),
                    requirement_level: RequirementLevel::Basic(
                        BasicRequirementLevelSpec::Recommended,
                    ),
                },
            }),
            annotations: None,
            constraints: None,
        };
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        // Stability is required for all types of values.
        group.body = Some(AnyValueSpec::String {
            common: AnyValueCommonSpec {
                id: "string_id".to_owned(),
                brief: "brief".to_owned(),
                note: "note".to_owned(),
                stability: None,
                examples: Some(Examples::String("test".to_owned())),
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            },
        });

        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(Error::InvalidAnyValue {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                value_id: "string_id".to_owned(),
                error: "Missing stability field.".to_owned(),
            },),
            result
        );

        // Stability is required for nested values.
        group.body = Some(AnyValueSpec::Map {
            common: AnyValueCommonSpec {
                id: "map_id".to_owned(),
                brief: "brief".to_owned(),
                note: "note".to_owned(),
                stability: Some(Stability::Stable),
                examples: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            },
            fields: vec![AnyValueSpec::String {
                common: AnyValueCommonSpec {
                    id: "nested_string_id".to_owned(),
                    brief: "brief".to_owned(),
                    note: "note".to_owned(),
                    stability: None,
                    examples: Some(Examples::String("test".to_owned())),
                    requirement_level: RequirementLevel::Basic(
                        BasicRequirementLevelSpec::Recommended,
                    ),
                },
            }],
        });
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(Error::InvalidAnyValue {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                value_id: "nested_string_id".to_owned(),
                error: "Missing stability field.".to_owned(),
            },),
            result
        );

        // Stability is required on enum members of nested values.
        group.body = Some(AnyValueSpec::Map {
            common: AnyValueCommonSpec {
                id: "map_id".to_owned(),
                brief: "brief".to_owned(),
                note: "note".to_owned(),
                stability: Some(Stability::Stable),
                examples: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            },
            fields: vec![AnyValueSpec::Enum {
                common: AnyValueCommonSpec {
                    id: "nested_enum".to_owned(),
                    brief: "brief".to_owned(),
                    note: "note".to_owned(),
                    stability: Some(Stability::Stable),
                    examples: None,
                    requirement_level: RequirementLevel::Basic(
                        BasicRequirementLevelSpec::Recommended,
                    ),
                },
                members: vec![EnumEntriesSpec {
                    id: "nested_enum_member".to_owned(),
                    value: ValueSpec::String("value".to_owned()),
                    brief: None,
                    note: None,
                    stability: None,
                    deprecated: None,
                }],
            }],
        });
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(Error::InvalidAnyValue {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                value_id: "nested_enum".to_owned(),
                error: "Missing stability field for enum member nested_enum_member.".to_owned(),
            },),
            result
        );
    }

    #[test]
    fn test_validate_group_stability() {
        let mut group = GroupSpec {
            id: "test".to_owned(),
            r#type: GroupType::AttributeGroup,
            brief: "test".to_owned(),
            note: "test".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: None,
            deprecated: None,
            attributes: vec![AttributeSpec::Id {
                id: "test".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: None,
                stability: Some(Stability::Development),
                deprecated: Some(Deprecated::Obsoleted {
                    note: "".to_owned(),
                }),
                examples: Some(Examples::String("test".to_owned())),
                tag: None,
                requirement_level: Default::default(),
                sampling_relevant: None,
                note: "".to_owned(),
                annotations: None,
            }],
            span_kind: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            name: None,
            display_name: None,
            body: None,
            annotations: None,
            constraints: None,
        };
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        // all other group types must have a stability field.
        group.r#type = GroupType::Span;
        group.span_kind = Some(SpanKindSpec::Client);
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupStability {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "This group does not contain a stability field.".to_owned(),
            }),
            result
        );
        group.stability = Some(Stability::Development);
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        group.stability = None;
        group.r#type = GroupType::Resource;
        group.span_kind = None;
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupStability {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "This group does not contain a stability field.".to_owned(),
            }),
            result
        );
        group.stability = Some(Stability::Development);
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        group.stability = None;

        group.r#type = GroupType::Scope;
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupStability {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "This group does not contain a stability field.".to_owned(),
            }),
            result
        );
        group.stability = Some(Stability::Development);
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        group.stability = None;

        group.r#type = GroupType::Metric;
        group.metric_name = Some("test".to_owned());
        group.instrument = Some(Counter);
        group.unit = Some("test".to_owned());
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupStability {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "This group does not contain a stability field.".to_owned(),
            }),
            result
        );
        group.stability = Some(Stability::Development);
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        group.stability = None;

        group.r#type = GroupType::Event;
        group.name = Some("test".to_owned());
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupStability {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "This group does not contain a stability field.".to_owned(),
            }),
            result
        );
        group.stability = Some(Stability::Development);
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        group.stability = None;

        group.r#type = GroupType::MetricGroup;
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupStability {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "This group does not contain a stability field.".to_owned(),
            }),
            result
        );

        group.stability = Some(Stability::Deprecated);
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupStability {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "Group stability is set to 'deprecated' which is no longer supported."
                    .to_owned(),
            }),
            result
        );

        group.stability = Some(Stability::Development);
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());
    }

    #[test]
    fn test_validate_extends_or_attributes() {
        let attributes = vec![AttributeSpec::Id {
            id: "test".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: None,
            stability: Some(Stability::Development),
            deprecated: Some(Deprecated::Obsoleted {
                note: "".to_owned(),
            }),
            examples: Some(Examples::String("test".to_owned())),
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: "".to_owned(),
            annotations: None,
        }];
        let mut group = GroupSpec {
            id: "test".to_owned(),
            r#type: GroupType::AttributeGroup,
            brief: "test".to_owned(),
            note: "test".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: Some(Stability::Stable),
            deprecated: None,
            attributes: vec![],
            span_kind: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            name: None,
            display_name: None,
            body: None,
            annotations: None,
            constraints: None,
        };

        // Attribute Group must have extends or attributes.
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupMissingExtendsOrAttributes {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "This group does not contain an extends or attributes field.".to_owned(),
            }),
            result
        );

        group.attributes = attributes.clone();
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        group.attributes = vec![];
        group.extends = Some("test".to_owned());
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());
        group.extends = None;

        // Span must have extends or attributes.
        group.r#type = GroupType::Span;
        group.span_kind = Some(SpanKindSpec::Client);
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupMissingExtendsOrAttributes {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "This group does not contain an extends or attributes field.".to_owned(),
            }),
            result
        );

        group.attributes = attributes.clone();
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        group.attributes = vec![];
        group.extends = Some("test".to_owned());
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());
        group.extends = None;

        // Resource must have extends or attributes.
        group.r#type = GroupType::Resource;
        group.span_kind = None;
        let result = group.validate("<test>").into_result_failing_non_fatal();
        assert_eq!(
            Err(InvalidGroupMissingExtendsOrAttributes {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                error: "This group does not contain an extends or attributes field.".to_owned(),
            }),
            result
        );

        group.attributes = attributes.clone();
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        group.attributes = vec![];
        group.extends = Some("test".to_owned());
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());
        group.extends = None;

        // Metrics DO NOT need extends or attributes.
        group.r#type = GroupType::Metric;
        group.metric_name = Some("test".to_owned());
        group.instrument = Some(Counter);
        group.unit = Some("test".to_owned());
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());

        // Events DO NOT need extends or attributes.
        group.r#type = GroupType::Event;
        group.name = Some("test".to_owned());
        assert!(group
            .validate("<test>")
            .into_result_failing_non_fatal()
            .is_ok());
    }

    #[test]
    fn test_validate_duplicate_attribute_ref() {
        let bad_attributes = vec![
            AttributeSpec::Ref {
                r#ref: "attribute".to_owned(),
                brief: None,
                examples: None,
                tag: None,
                requirement_level: None,
                sampling_relevant: None,
                note: None,
                stability: None,
                deprecated: None,
                prefix: false,
                annotations: None,
            },
            AttributeSpec::Ref {
                r#ref: "attribute".to_owned(),
                brief: None,
                examples: None,
                tag: None,
                requirement_level: None,
                sampling_relevant: None,
                note: None,
                stability: None,
                deprecated: None,
                prefix: false,
                annotations: None,
            },
        ];
        let mut group = GroupSpec {
            id: "test".to_owned(),
            r#type: GroupType::AttributeGroup,
            brief: "test".to_owned(),
            note: "test".to_owned(),
            prefix: "".to_owned(),
            extends: None,
            stability: Some(Stability::Stable),
            deprecated: None,
            attributes: vec![],
            span_kind: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            name: None,
            display_name: None,
            body: None,
            annotations: None,
            constraints: None,
        };

        // Check group with duplicate attributes.
        group.attributes = bad_attributes.clone();
        let result = group.validate("<test>");
        assert_eq!(
            Err(Error::InvalidGroupDuplicateAttributeRef {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                attribute_ref: "attribute".to_owned(),
            }),
            result.into_result_failing_non_fatal()
        );
    }

    #[test]
    fn test_instrumentation_spec() {
        assert_eq!(Counter.to_string(), "counter");
        assert_eq!(Gauge.to_string(), "gauge");
        assert_eq!(Histogram.to_string(), "histogram");
        assert_eq!(UpDownCounter.to_string(), "updowncounter");
    }
}

/// A group spec with its provenance (path or URL).
#[derive(Debug, Clone)]
pub struct GroupSpecWithProvenance {
    /// The group spec.
    pub spec: GroupSpec,
    /// The provenance of the group spec (path or URL).
    pub provenance: String,
}
