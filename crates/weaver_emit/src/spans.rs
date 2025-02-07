// SPDX-License-Identifier: Apache-2.0

//! Translations from Weaver to Otel for spans.

use opentelemetry::{
    global,
    trace::{SpanKind, TraceContextExt, Tracer},
    Array, KeyValue, Value,
};
use weaver_forge::registry::ResolvedRegistry;
use weaver_resolved_schema::attribute::Attribute;
use weaver_semconv::{
    attribute::{AttributeType, Examples, PrimitiveOrArrayTypeSpec, TemplateTypeSpec},
    group::{GroupType, SpanKindSpec},
};

// TODO These constants should be replaced with official semconvs when available.
const WEAVER_EMIT_SPAN: &str = "otel.weaver.emit";
const WEAVER_REGISTRY_PATH: &str = "otel.weaver.registry_path";

/// Convert the Weaver span kind to an OTLP span kind.
/// If the span kind is not specified, return `SpanKind::Internal`.
#[must_use]
fn otel_span_kind(span_kind: Option<&SpanKindSpec>) -> SpanKind {
    match span_kind {
        Some(SpanKindSpec::Client) => SpanKind::Client,
        Some(SpanKindSpec::Server) => SpanKind::Server,
        Some(SpanKindSpec::Producer) => SpanKind::Producer,
        Some(SpanKindSpec::Consumer) => SpanKind::Consumer,
        Some(SpanKindSpec::Internal) | None => SpanKind::Internal,
    }
}

/// For the given attribute, return a name/value pair.
/// Values are generated based on the attribute type and examples where possible.
#[must_use]
fn get_attribute_name_value(attribute: &Attribute) -> KeyValue {
    let name = attribute.name.clone();
    match &attribute.r#type {
        AttributeType::PrimitiveOrArray(primitive_or_array) => {
            let value = match primitive_or_array {
                PrimitiveOrArrayTypeSpec::Boolean => Value::Bool(true),
                PrimitiveOrArrayTypeSpec::Int => match &attribute.examples {
                    Some(Examples::Int(i)) => Value::I64(*i),
                    Some(Examples::Ints(ints)) => Value::I64(*ints.first().unwrap_or(&42)),
                    _ => Value::I64(42),
                },
                PrimitiveOrArrayTypeSpec::Double => match &attribute.examples {
                    Some(Examples::Double(d)) => Value::F64(f64::from(*d)),
                    Some(Examples::Doubles(doubles)) => {
                        Value::F64(f64::from(*doubles.first().unwrap_or((&3.13).into())))
                    }
                    _ => Value::F64(3.13),
                },
                PrimitiveOrArrayTypeSpec::String => match &attribute.examples {
                    Some(Examples::String(s)) => Value::String(s.clone().into()),
                    Some(Examples::Strings(strings)) => Value::String(
                        strings
                            .first()
                            .unwrap_or(&"value".to_owned())
                            .clone()
                            .into(),
                    ),
                    _ => Value::String("value".into()),
                },
                PrimitiveOrArrayTypeSpec::Booleans => Value::Array(Array::Bool(vec![true, false])),
                PrimitiveOrArrayTypeSpec::Ints => match &attribute.examples {
                    Some(Examples::Ints(ints)) => Value::Array(Array::I64(ints.to_vec())),
                    Some(Examples::ListOfInts(list_of_ints)) => Value::Array(Array::I64(
                        list_of_ints.first().unwrap_or(&vec![42, 43]).to_vec(),
                    )),
                    _ => Value::Array(Array::I64(vec![42, 43])),
                },
                PrimitiveOrArrayTypeSpec::Doubles => match &attribute.examples {
                    Some(Examples::Doubles(doubles)) => {
                        Value::Array(Array::F64(doubles.iter().map(|d| f64::from(*d)).collect()))
                    }
                    Some(Examples::ListOfDoubles(list_of_doubles)) => Value::Array(Array::F64(
                        list_of_doubles
                            .first()
                            .unwrap_or(&vec![(3.13).into(), (3.15).into()])
                            .iter()
                            .map(|d| f64::from(*d))
                            .collect(),
                    )),
                    _ => Value::Array(Array::F64(vec![3.13, 3.15])),
                },
                PrimitiveOrArrayTypeSpec::Strings => match &attribute.examples {
                    Some(Examples::Strings(strings)) => Value::Array(Array::String(
                        strings.iter().map(|s| s.clone().into()).collect(),
                    )),
                    Some(Examples::ListOfStrings(list_of_strings)) => Value::Array(Array::String(
                        list_of_strings
                            .first()
                            .unwrap_or(&vec!["value1".to_owned(), "value2".to_owned()])
                            .iter()
                            .map(|s| s.clone().into())
                            .collect(),
                    )),
                    _ => Value::Array(Array::String(vec!["value1".into(), "value2".into()])),
                },
            };
            KeyValue::new(name, value)
        }
        AttributeType::Enum { members, .. } => {
            KeyValue::new(name, Value::String(members[0].value.to_string().into()))
        }
        AttributeType::Template(template_type_spec) => {
            // TODO Support examples when https://github.com/open-telemetry/semantic-conventions/issues/1740 is complete
            let value = match template_type_spec {
                TemplateTypeSpec::String => Value::String("template_value".into()),
                TemplateTypeSpec::Int => Value::I64(42),
                TemplateTypeSpec::Double => Value::F64(3.13),
                TemplateTypeSpec::Boolean => Value::Bool(true),
                TemplateTypeSpec::Strings => Value::Array(Array::String(vec![
                    "template_value1".into(),
                    "template_value2".into(),
                ])),
                TemplateTypeSpec::Ints => Value::Array(Array::I64(vec![42, 43])),
                TemplateTypeSpec::Doubles => Value::Array(Array::F64(vec![3.13, 3.15])),
                TemplateTypeSpec::Booleans => Value::Array(Array::Bool(vec![true, false])),
            };
            KeyValue::new(format!("{name}.key"), value)
        }
    }
}

/// Uses the global tracer_provider to emit a single trace for all the defined
/// spans in the registry
pub(crate) fn emit_trace_for_registry(registry: &ResolvedRegistry, registry_path: &str) {
    let tracer = global::tracer("weaver");
    // Start a parent span here and use this context to create child spans
    tracer.in_span(WEAVER_EMIT_SPAN, |cx| {
        let span = cx.span();
        span.set_attribute(KeyValue::new(
            WEAVER_REGISTRY_PATH,
            registry_path.to_owned(),
        ));

        // Emit each span to the OTLP receiver.
        for group in registry.groups.iter() {
            if group.r#type == GroupType::Span {
                let _span = tracer
                    .span_builder(group.id.clone())
                    .with_kind(otel_span_kind(group.span_kind.as_ref()))
                    .with_attributes(group.attributes.iter().map(get_attribute_name_value))
                    .start_with_context(&tracer, &cx);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use opentelemetry::KeyValue;
    use opentelemetry::{global, Array, Value};
    use opentelemetry_sdk::{trace as sdktrace, Resource};

    use futures_util::future::BoxFuture;
    use opentelemetry::trace::SpanKind;
    use opentelemetry_sdk::export::{self, trace::ExportResult};
    use ordered_float::OrderedFloat;
    use std::sync::{Arc, Mutex};
    use weaver_forge::registry::{ResolvedGroup, ResolvedRegistry};
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::attribute::{
        AttributeType, EnumEntriesSpec, Examples, PrimitiveOrArrayTypeSpec, RequirementLevel,
        TemplateTypeSpec, ValueSpec,
    };
    use weaver_semconv::group::{GroupType, SpanKindSpec};
    use weaver_semconv::stability::Stability;

    use super::{WEAVER_EMIT_SPAN, WEAVER_REGISTRY_PATH};

    #[derive(Debug)]
    pub struct SpanExporter {
        // resource: Resource,
        // is_shutdown: atomic::AtomicBool,
        spans: Arc<Mutex<Vec<export::trace::SpanData>>>,
    }

    impl export::trace::SpanExporter for SpanExporter {
        fn export(
            &mut self,
            batch: Vec<export::trace::SpanData>,
        ) -> BoxFuture<'static, ExportResult> {
            self.spans.lock().unwrap().extend(batch);
            Box::pin(std::future::ready(Ok(())))
        }
    }

    #[test]
    fn test_emit_trace_for_registry() {
        let spans = Arc::new(Mutex::new(Vec::new()));
        let span_exporter = SpanExporter {
            spans: spans.clone(),
        };
        let tracer_provider = sdktrace::TracerProvider::builder()
            .with_resource(Resource::new(vec![KeyValue::new("service.name", "weaver")]))
            .with_simple_exporter(span_exporter)
            .build();

        let _ = global::set_tracer_provider(tracer_provider.clone());

        let registry = ResolvedRegistry {
            registry_url: "TEST".to_owned(),
            groups: vec![
                ResolvedGroup {
                    id: "test.comprehensive.client".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    constraints: vec![],
                    attributes: vec![
                        Attribute {
                            name: "test.string".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: Some(Examples::Strings(vec![
                                "value1".to_owned(),
                                "value2".to_owned(),
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.integer".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
                            examples: Some(Examples::Ints(vec![42, 123])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.double".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::Double,
                            ),
                            examples: Some(Examples::Doubles(vec![
                                OrderedFloat(3.13),
                                OrderedFloat(2.71),
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.boolean".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::Boolean,
                            ),
                            examples: Some(Examples::Bools(vec![true, false])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.string_array".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::Strings,
                            ),
                            examples: Some(Examples::ListOfStrings(vec![
                                vec!["val1".to_owned(), "val2".to_owned()],
                                vec!["val3".to_owned(), "val4".to_owned()],
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.int_array".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints),
                            examples: Some(Examples::ListOfInts(vec![vec![1, 2], vec![3, 4]])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.double_array".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::Doubles,
                            ),
                            examples: Some(Examples::ListOfDoubles(vec![
                                vec![OrderedFloat(1.1), OrderedFloat(2.2)],
                                vec![OrderedFloat(3.3), OrderedFloat(4.4)],
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.boolean_array".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::Booleans,
                            ),
                            examples: Some(Examples::ListOfBools(vec![
                                vec![true, false],
                                vec![false, true],
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.template_string".to_owned(),
                            r#type: AttributeType::Template(TemplateTypeSpec::String),
                            examples: Some(Examples::Strings(vec![
                                "test.template_string.key=\"hello\"".to_owned(),
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.template_string_array".to_owned(),
                            r#type: AttributeType::Template(TemplateTypeSpec::Strings),
                            examples: Some(Examples::Strings(vec![
                                "test.template_string_array.key=[\"val1\",\"val2\"]".to_owned(),
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.enum".to_owned(),
                            r#type: AttributeType::Enum {
                                allow_custom_values: None,
                                members: vec![
                                    EnumEntriesSpec {
                                        id: "value1".to_owned(),
                                        value: ValueSpec::String("VALUE_1".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Stable),
                                        deprecated: None,
                                    },
                                    EnumEntriesSpec {
                                        id: "value2".to_owned(),
                                        value: ValueSpec::String("VALUE_2".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Stable),
                                        deprecated: None,
                                    },
                                ],
                            },
                            examples: None,
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                    ],
                    span_kind: Some(SpanKindSpec::Client),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                },
                ResolvedGroup {
                    id: "test.comprehensive.server".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    constraints: vec![],
                    attributes: vec![Attribute {
                        name: "test.string".to_owned(),
                        r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                        examples: Some(Examples::String("value1".to_owned())),
                        brief: "".to_owned(),
                        tag: None,
                        requirement_level: RequirementLevel::Recommended {
                            text: "".to_owned(),
                        },
                        sampling_relevant: None,
                        note: "".to_owned(),
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        prefix: false,
                        tags: None,
                        value: None,
                    }],
                    span_kind: Some(SpanKindSpec::Server),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                },
                ResolvedGroup {
                    id: "test.comprehensive.producer".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    constraints: vec![],
                    attributes: vec![Attribute {
                        name: "test.int".to_owned(),
                        r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
                        examples: Some(Examples::Int(42)),
                        brief: "".to_owned(),
                        tag: None,
                        requirement_level: RequirementLevel::Recommended {
                            text: "".to_owned(),
                        },
                        sampling_relevant: None,
                        note: "".to_owned(),
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        prefix: false,
                        tags: None,
                        value: None,
                    }],
                    span_kind: Some(SpanKindSpec::Producer),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                },
                ResolvedGroup {
                    id: "test.comprehensive.consumer".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    constraints: vec![],
                    attributes: vec![Attribute {
                        name: "test.double".to_owned(),
                        r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double),
                        examples: Some(Examples::Double(OrderedFloat(3.13))),
                        brief: "".to_owned(),
                        tag: None,
                        requirement_level: RequirementLevel::Recommended {
                            text: "".to_owned(),
                        },
                        sampling_relevant: None,
                        note: "".to_owned(),
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        prefix: false,
                        tags: None,
                        value: None,
                    }],
                    span_kind: Some(SpanKindSpec::Consumer),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                },
                ResolvedGroup {
                    id: "test.comprehensive.internal".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    constraints: vec![],
                    attributes: vec![
                        Attribute {
                            name: "test.string_array".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::Strings,
                            ),
                            examples: Some(Examples::Strings(vec![
                                "val1".to_owned(),
                                "val2".to_owned(),
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.int_array".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints),
                            examples: Some(Examples::Ints(vec![1, 2])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.double_array".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::Doubles,
                            ),
                            examples: Some(Examples::Doubles(vec![
                                OrderedFloat(1.1),
                                OrderedFloat(2.2),
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                    ],
                    span_kind: Some(SpanKindSpec::Internal),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                },
                ResolvedGroup {
                    id: "test.no.examples".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    constraints: vec![],
                    attributes: vec![
                        Attribute {
                            name: "test.string".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: None,
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.integer".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
                            examples: None,
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.double".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::Double,
                            ),
                            examples: None,
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.string_array".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::Strings,
                            ),
                            examples: None,
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.int_array".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints),
                            examples: None,
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.double_array".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::Doubles,
                            ),
                            examples: None,
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.template_string".to_owned(),
                            r#type: AttributeType::Template(TemplateTypeSpec::String),
                            examples: None,
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                        Attribute {
                            name: "test.template_string_array".to_owned(),
                            r#type: AttributeType::Template(TemplateTypeSpec::Strings),
                            examples: None,
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                        },
                    ],
                    span_kind: Some(SpanKindSpec::Client),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                },
            ],
        };
        super::emit_trace_for_registry(&registry, "TEST");
        global::shutdown_tracer_provider();

        // Now check the spans stored in the span exporter
        assert_eq!(spans.lock().unwrap().len(), 7);

        let expected = vec![
            (
                "test.comprehensive.client",
                SpanKind::Client,
                vec![
                    KeyValue::new("test.string", "value1".to_owned()),
                    KeyValue::new("test.integer", Value::I64(42)),
                    KeyValue::new("test.double", Value::F64(3.13)),
                    KeyValue::new("test.boolean", Value::Bool(true)),
                    KeyValue::new(
                        "test.string_array",
                        Value::Array(Array::String(vec!["val1".into(), "val2".into()])),
                    ),
                    KeyValue::new("test.int_array", Value::Array(Array::I64(vec![1, 2]))),
                    KeyValue::new(
                        "test.double_array",
                        Value::Array(Array::F64(vec![1.1, 2.2])),
                    ),
                    KeyValue::new(
                        "test.boolean_array",
                        Value::Array(Array::Bool(vec![true, false])),
                    ),
                    KeyValue::new(
                        "test.template_string.key",
                        Value::String("template_value".into()),
                    ),
                    KeyValue::new(
                        "test.template_string_array.key",
                        Value::Array(Array::String(vec![
                            "template_value1".into(),
                            "template_value2".into(),
                        ])),
                    ),
                    KeyValue::new("test.enum", Value::String("VALUE_1".into())),
                ],
            ),
            (
                "test.comprehensive.server",
                SpanKind::Server,
                vec![KeyValue::new("test.string", Value::String("value1".into()))],
            ),
            (
                "test.comprehensive.producer",
                SpanKind::Producer,
                vec![KeyValue::new("test.int", Value::I64(42))],
            ),
            (
                "test.comprehensive.consumer",
                SpanKind::Consumer,
                vec![KeyValue::new("test.double", Value::F64(3.13))],
            ),
            (
                "test.comprehensive.internal",
                SpanKind::Internal,
                vec![
                    KeyValue::new(
                        "test.string_array",
                        Value::Array(Array::String(vec!["val1".into(), "val2".into()])),
                    ),
                    KeyValue::new("test.int_array", Value::Array(Array::I64(vec![1, 2]))),
                    KeyValue::new(
                        "test.double_array",
                        Value::Array(Array::F64(vec![1.1, 2.2])),
                    ),
                ],
            ),
            (
                "test.no.examples",
                SpanKind::Client,
                vec![
                    KeyValue::new("test.string", "value".to_owned()),
                    KeyValue::new("test.integer", Value::I64(42)),
                    KeyValue::new("test.double", Value::F64(3.13)),
                    KeyValue::new(
                        "test.string_array",
                        Value::Array(Array::String(vec!["value1".into(), "value2".into()])),
                    ),
                    KeyValue::new("test.int_array", Value::Array(Array::I64(vec![42, 43]))),
                    KeyValue::new(
                        "test.double_array",
                        Value::Array(Array::F64(vec![3.13, 3.15])),
                    ),
                    KeyValue::new(
                        "test.template_string.key",
                        Value::String("template_value".into()),
                    ),
                    KeyValue::new(
                        "test.template_string_array.key",
                        Value::Array(Array::String(vec![
                            "template_value1".into(),
                            "template_value2".into(),
                        ])),
                    ),
                ],
            ),
            (
                WEAVER_EMIT_SPAN,
                SpanKind::Internal,
                vec![KeyValue::new(
                    WEAVER_REGISTRY_PATH,
                    Value::String("TEST".into()),
                )],
            ),
        ];
        for (i, span_data) in spans.lock().unwrap().iter().enumerate() {
            assert_eq!(span_data.name, expected[i].0);
            assert_eq!(span_data.span_kind, expected[i].1);
            for (j, attr) in span_data.attributes.iter().enumerate() {
                assert_eq!(attr.key, expected[i].2[j].key);
                assert_eq!(attr.value, expected[i].2[j].value);
            }
        }
    }
}
