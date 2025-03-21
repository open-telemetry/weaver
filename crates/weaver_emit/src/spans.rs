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
    use super::*;
    use opentelemetry::{Array, KeyValue, Value};
    use ordered_float::OrderedFloat;
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::attribute::{
        AttributeType, EnumEntriesSpec, Examples, PrimitiveOrArrayTypeSpec, RequirementLevel,
        TemplateTypeSpec, ValueSpec,
    };
    use weaver_semconv::stability::Stability;

    fn create_test_attribute(
        name: &str,
        attr_type: AttributeType,
        examples: Option<Examples>,
    ) -> Attribute {
        Attribute {
            name: name.to_owned(),
            r#type: attr_type,
            examples,
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
            annotations: None,
        }
    }

    #[test]
    fn test_primitive_boolean() {
        let attr = create_test_attribute(
            "test.bool",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.bool", true));
    }

    #[test]
    fn test_primitive_int_with_example() {
        let attr = create_test_attribute(
            "test.int",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
            Some(Examples::Int(123)),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.int", 123_i64));
    }

    #[test]
    fn test_primitive_int_with_ints_example() {
        let attr = create_test_attribute(
            "test.int",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
            Some(Examples::Ints(vec![42, 43, 44])),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.int", 42_i64));
    }

    #[test]
    fn test_primitive_int_without_example() {
        let attr = create_test_attribute(
            "test.int",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.int", 42_i64));
    }

    #[test]
    fn test_primitive_double_with_example() {
        let attr = create_test_attribute(
            "test.double",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double),
            Some(Examples::Double(OrderedFloat(3.15))),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.double", 3.15));
    }

    #[test]
    fn test_primitive_double_with_doubles_example() {
        let attr = create_test_attribute(
            "test.double",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double),
            Some(Examples::Doubles(vec![
                OrderedFloat(3.15),
                OrderedFloat(2.71),
            ])),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.double", 3.15));
    }

    #[test]
    fn test_primitive_double_without_example() {
        let attr = create_test_attribute(
            "test.double",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.double", 3.13));
    }

    #[test]
    fn test_primitive_string_with_example() {
        let attr = create_test_attribute(
            "test.string",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            Some(Examples::String("example".to_owned())),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.string", "example"));
    }

    #[test]
    fn test_primitive_string_without_example() {
        let attr = create_test_attribute(
            "test.string",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.string", "value"));
    }

    #[test]
    fn test_array_boolean() {
        let attr = create_test_attribute(
            "test.bools",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Booleans),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv.value, Value::Array(Array::Bool(vec![true, false])));
    }

    #[test]
    fn test_array_ints_with_example() {
        let attr = create_test_attribute(
            "test.ints",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints),
            Some(Examples::Ints(vec![1, 2, 3])),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv.value, Value::Array(Array::I64(vec![1, 2, 3])));
    }

    #[test]
    fn test_array_ints_with_list_example() {
        let attr = create_test_attribute(
            "test.ints",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints),
            Some(Examples::ListOfInts(vec![vec![1, 2, 3], vec![4, 5, 6]])),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv.value, Value::Array(Array::I64(vec![1, 2, 3])));
    }

    #[test]
    fn test_array_ints_without_example() {
        let attr = create_test_attribute(
            "test.ints",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Ints),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv.value, Value::Array(Array::I64(vec![42, 43])));
    }

    #[test]
    fn test_array_doubles_with_example() {
        let attr = create_test_attribute(
            "test.doubles",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Doubles),
            Some(Examples::Doubles(vec![
                OrderedFloat(1.1),
                OrderedFloat(2.2),
            ])),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv.value, Value::Array(Array::F64(vec![1.1, 2.2])));
    }

    #[test]
    fn test_array_doubles_with_list_example() {
        let attr = create_test_attribute(
            "test.doubles",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Doubles),
            Some(Examples::ListOfDoubles(vec![
                vec![OrderedFloat(1.1), OrderedFloat(2.2)],
                vec![OrderedFloat(3.3), OrderedFloat(4.4)],
            ])),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv.value, Value::Array(Array::F64(vec![1.1, 2.2])));
    }

    #[test]
    fn test_array_doubles_without_example() {
        let attr = create_test_attribute(
            "test.doubles",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Doubles),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv.value, Value::Array(Array::F64(vec![3.13, 3.15])));
    }

    #[test]
    fn test_array_strings_with_example() {
        let attr = create_test_attribute(
            "test.strings",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings),
            Some(Examples::Strings(vec!["one".to_owned(), "two".to_owned()])),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(
            kv.value,
            Value::Array(Array::String(vec!["one".into(), "two".into()]))
        );
    }

    #[test]
    fn test_array_strings_with_list_example() {
        let attr = create_test_attribute(
            "test.strings",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings),
            Some(Examples::ListOfStrings(vec![
                vec!["a".to_owned(), "b".to_owned()],
                vec!["c".to_owned(), "d".to_owned()],
            ])),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(
            kv.value,
            Value::Array(Array::String(vec!["a".into(), "b".into()]))
        );
    }

    #[test]
    fn test_array_strings_without_example() {
        let attr = create_test_attribute(
            "test.strings",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(
            kv.value,
            Value::Array(Array::String(vec!["value1".into(), "value2".into()]))
        );
    }

    #[test]
    fn test_enum() {
        let attr = create_test_attribute(
            "test.enum",
            AttributeType::Enum {
                allow_custom_values: None,
                members: vec![
                    EnumEntriesSpec {
                        id: "first".to_owned(),
                        value: ValueSpec::String("FIRST".to_owned()),
                        brief: None,
                        note: None,
                        stability: None,
                        deprecated: None,
                    },
                    EnumEntriesSpec {
                        id: "second".to_owned(),
                        value: ValueSpec::String("SECOND".to_owned()),
                        brief: None,
                        note: None,
                        stability: None,
                        deprecated: None,
                    },
                ],
            },
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.enum", "FIRST"));
    }

    #[test]
    fn test_template_string() {
        let attr = create_test_attribute(
            "test.template",
            AttributeType::Template(TemplateTypeSpec::String),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.template.key", "template_value"));
    }

    #[test]
    fn test_template_int() {
        let attr = create_test_attribute(
            "test.template",
            AttributeType::Template(TemplateTypeSpec::Int),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.template.key", 42_i64));
    }

    #[test]
    fn test_template_double() {
        let attr = create_test_attribute(
            "test.template",
            AttributeType::Template(TemplateTypeSpec::Double),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.template.key", 3.13));
    }

    #[test]
    fn test_template_boolean() {
        let attr = create_test_attribute(
            "test.template",
            AttributeType::Template(TemplateTypeSpec::Boolean),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.template.key", true));
    }

    #[test]
    fn test_template_strings() {
        let attr = create_test_attribute(
            "test.template",
            AttributeType::Template(TemplateTypeSpec::Strings),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(
            kv.value,
            Value::Array(Array::String(vec![
                "template_value1".into(),
                "template_value2".into()
            ]))
        );
    }

    #[test]
    fn test_template_ints() {
        let attr = create_test_attribute(
            "test.template",
            AttributeType::Template(TemplateTypeSpec::Ints),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv.value, Value::Array(Array::I64(vec![42, 43])));
    }

    #[test]
    fn test_template_doubles() {
        let attr = create_test_attribute(
            "test.template",
            AttributeType::Template(TemplateTypeSpec::Doubles),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv.value, Value::Array(Array::F64(vec![3.13, 3.15])));
    }

    #[test]
    fn test_template_booleans() {
        let attr = create_test_attribute(
            "test.template",
            AttributeType::Template(TemplateTypeSpec::Booleans),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv.value, Value::Array(Array::Bool(vec![true, false])));
    }

    #[test]
    fn test_span_kinds() {
        assert_eq!(
            otel_span_kind(Some(&SpanKindSpec::Client)),
            SpanKind::Client
        );
        assert_eq!(
            otel_span_kind(Some(&SpanKindSpec::Server)),
            SpanKind::Server
        );
        assert_eq!(
            otel_span_kind(Some(&SpanKindSpec::Producer)),
            SpanKind::Producer
        );
        assert_eq!(
            otel_span_kind(Some(&SpanKindSpec::Consumer)),
            SpanKind::Consumer
        );
        assert_eq!(
            otel_span_kind(Some(&SpanKindSpec::Internal)),
            SpanKind::Internal
        );
        assert_eq!(otel_span_kind(None), SpanKind::Internal);
    }
}
