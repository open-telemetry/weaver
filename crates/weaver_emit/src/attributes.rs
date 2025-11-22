// SPDX-License-Identifier: Apache-2.0

//! Translations from Weaver to Otel for attributes.

use opentelemetry::{Array, KeyValue, Value};
use weaver_resolved_schema::attribute::Attribute;
use weaver_semconv::attribute::ValueSpec;
use weaver_semconv::attribute::{
    AttributeType, Examples, PrimitiveOrArrayTypeSpec, TemplateTypeSpec,
};

/// For the given attribute, return a name/value pair.
/// Values are generated based on the attribute type and examples where possible.
#[must_use]
pub fn get_attribute_name_value(attribute: &Attribute) -> KeyValue {
    internal_get_attribute_name_value(
        attribute.name.clone(),
        &attribute.r#type,
        attribute.examples.as_ref(),
    )
}

/// For the given attribute, return a name/value pair.
/// Values are generated based on the attribute type and examples where possible.
#[must_use]
pub fn get_attribute_name_value_v2(attribute: &weaver_forge::v2::attribute::Attribute) -> KeyValue {
    internal_get_attribute_name_value(
        attribute.key.clone(),
        &attribute.r#type,
        attribute.examples.as_ref(),
    )
}

fn internal_get_attribute_name_value(
    name: String,
    r#type: &AttributeType,
    examples: Option<&Examples>,
) -> KeyValue {
    match r#type {
        AttributeType::PrimitiveOrArray(primitive_or_array) => {
            let value = match primitive_or_array {
                PrimitiveOrArrayTypeSpec::Boolean => Value::Bool(true),
                PrimitiveOrArrayTypeSpec::Int => match &examples {
                    Some(Examples::Int(i)) => Value::I64(*i),
                    Some(Examples::Ints(ints)) => Value::I64(*ints.first().unwrap_or(&42)),
                    _ => Value::I64(42),
                },
                PrimitiveOrArrayTypeSpec::Double => match &examples {
                    Some(Examples::Double(d)) => Value::F64(f64::from(*d)),
                    Some(Examples::Doubles(doubles)) => {
                        Value::F64(f64::from(*doubles.first().unwrap_or((&3.13).into())))
                    }
                    _ => Value::F64(3.13),
                },
                PrimitiveOrArrayTypeSpec::String => match &examples {
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
                PrimitiveOrArrayTypeSpec::Any => match &examples {
                    // Boolean-based examples
                    Some(Examples::Bool(b)) => Value::Bool(*b),
                    Some(Examples::Bools(booleans)) => {
                        Value::Bool(*booleans.first().unwrap_or(&true))
                    }
                    Some(Examples::ListOfBools(list_of_bools)) => Value::Array(Array::Bool(
                        list_of_bools.first().unwrap_or(&vec![true, false]).to_vec(),
                    )),
                    // Integer-based examples
                    Some(Examples::Int(i)) => Value::I64(*i),
                    Some(Examples::Ints(ints)) => Value::I64(*ints.first().unwrap_or(&42)),
                    Some(Examples::ListOfInts(list_of_ints)) => Value::Array(Array::I64(
                        list_of_ints.first().unwrap_or(&vec![42, 43]).to_vec(),
                    )),
                    // Double-based examples
                    Some(Examples::Double(d)) => Value::F64(f64::from(*d)),
                    Some(Examples::Doubles(doubles)) => {
                        Value::F64(f64::from(*doubles.first().unwrap_or((&3.13).into())))
                    }
                    Some(Examples::ListOfDoubles(list_of_doubles)) => Value::Array(Array::F64(
                        list_of_doubles
                            .first()
                            .unwrap_or(&vec![(3.13).into(), (3.15).into()])
                            .iter()
                            .map(|d| f64::from(*d))
                            .collect(),
                    )),
                    // String-based examples
                    Some(Examples::String(s)) => Value::String(s.clone().into()),
                    Some(Examples::Strings(strings)) => Value::String(
                        strings
                            .first()
                            .unwrap_or(&"value".to_owned())
                            .clone()
                            .into(),
                    ),
                    Some(Examples::ListOfStrings(list_of_strings)) => Value::Array(Array::String(
                        list_of_strings
                            .first()
                            .unwrap_or(&vec!["value1".to_owned(), "value2".to_owned()])
                            .iter()
                            .map(|s| s.clone().into())
                            .collect(),
                    )),
                    Some(Examples::Any(any)) => match any {
                        ValueSpec::Int(v) => Value::I64(*v),
                        ValueSpec::Double(v) => Value::F64(f64::from(*v)),
                        ValueSpec::String(v) => Value::String(v.clone().into()),
                        ValueSpec::Bool(v) => Value::Bool(*v),
                    },
                    Some(Examples::Anys(anys)) => anys
                        .first()
                        .map(|v| match v {
                            ValueSpec::Int(v) => Value::I64(*v),
                            ValueSpec::Double(v) => Value::F64(f64::from(*v)),
                            ValueSpec::String(v) => Value::String(v.clone().into()),
                            ValueSpec::Bool(v) => Value::Bool(*v),
                        })
                        .unwrap_or(Value::String("value".into())),
                    // Fallback to a default value
                    _ => Value::String("value".into()),
                },
                PrimitiveOrArrayTypeSpec::Booleans => Value::Array(Array::Bool(vec![true, false])),
                PrimitiveOrArrayTypeSpec::Ints => match &examples {
                    Some(Examples::Ints(ints)) => Value::Array(Array::I64(ints.to_vec())),
                    Some(Examples::ListOfInts(list_of_ints)) => Value::Array(Array::I64(
                        list_of_ints.first().unwrap_or(&vec![42, 43]).to_vec(),
                    )),
                    _ => Value::Array(Array::I64(vec![42, 43])),
                },
                PrimitiveOrArrayTypeSpec::Doubles => match &examples {
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
                PrimitiveOrArrayTypeSpec::Strings => match &examples {
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
            let value = match &members[0].value {
                ValueSpec::String(s) => Value::String(s.clone().into()),
                ValueSpec::Int(i) => Value::I64(*i),
                ValueSpec::Double(d) => Value::F64(f64::from(*d)),
                ValueSpec::Bool(b) => Value::Bool(*b),
            };
            KeyValue::new(name, value)
        }
        AttributeType::Template(template_type_spec) => {
            // TODO Support examples when https://github.com/open-telemetry/semantic-conventions/issues/1740 is complete
            let value = match template_type_spec {
                TemplateTypeSpec::String => Value::String("template_value".into()),
                TemplateTypeSpec::Int => Value::I64(42),
                TemplateTypeSpec::Double => Value::F64(3.13),
                TemplateTypeSpec::Boolean => Value::Bool(true),
                TemplateTypeSpec::Any => Value::String("template_any_value".into()),
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
            role: Default::default(),
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
    fn test_primitive_any_with_example() {
        let attr = create_test_attribute(
            "test.any",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Any),
            Some(Examples::Anys(vec![
                ValueSpec::Int(123),
                ValueSpec::String("example".to_owned()),
            ])),
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.any", 123));
    }

    #[test]
    fn test_primitive_any_without_example() {
        let attr = create_test_attribute(
            "test.any",
            AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Any),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.any", "value"));
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
    fn test_enum_string() {
        let attr = create_test_attribute(
            "test.enum",
            AttributeType::Enum {
                members: vec![
                    EnumEntriesSpec {
                        id: "first".to_owned(),
                        value: ValueSpec::String("FIRST".to_owned()),
                        brief: None,
                        note: None,
                        stability: None,
                        deprecated: None,
                        annotations: None,
                    },
                    EnumEntriesSpec {
                        id: "second".to_owned(),
                        value: ValueSpec::String("SECOND".to_owned()),
                        brief: None,
                        note: None,
                        stability: None,
                        deprecated: None,
                        annotations: None,
                    },
                ],
            },
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.enum", "FIRST"));
    }

    #[test]
    fn test_enum_int() {
        let attr = create_test_attribute(
            "test.enum",
            AttributeType::Enum {
                members: vec![
                    EnumEntriesSpec {
                        id: "first".to_owned(),
                        value: ValueSpec::Int(1),
                        brief: None,
                        note: None,
                        stability: None,
                        deprecated: None,
                        annotations: None,
                    },
                    EnumEntriesSpec {
                        id: "second".to_owned(),
                        value: ValueSpec::Int(2),
                        brief: None,
                        note: None,
                        stability: None,
                        deprecated: None,
                        annotations: None,
                    },
                ],
            },
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.enum", 1_i64));
    }

    #[test]
    fn test_enum_double() {
        let attr = create_test_attribute(
            "test.enum",
            AttributeType::Enum {
                members: vec![
                    EnumEntriesSpec {
                        id: "first".to_owned(),
                        value: ValueSpec::Double(OrderedFloat(1.5)),
                        brief: None,
                        note: None,
                        stability: None,
                        deprecated: None,
                        annotations: None,
                    },
                    EnumEntriesSpec {
                        id: "second".to_owned(),
                        value: ValueSpec::Double(OrderedFloat(2.5)),
                        brief: None,
                        note: None,
                        stability: None,
                        deprecated: None,
                        annotations: None,
                    },
                ],
            },
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.enum", 1.5));
    }

    #[test]
    fn test_enum_boolean() {
        let attr = create_test_attribute(
            "test.enum",
            AttributeType::Enum {
                members: vec![
                    EnumEntriesSpec {
                        id: "first".to_owned(),
                        value: ValueSpec::Bool(true),
                        brief: None,
                        note: None,
                        stability: None,
                        deprecated: None,
                        annotations: None,
                    },
                    EnumEntriesSpec {
                        id: "second".to_owned(),
                        value: ValueSpec::Bool(false),
                        brief: None,
                        note: None,
                        stability: None,
                        deprecated: None,
                        annotations: None,
                    },
                ],
            },
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.enum", true));
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
    fn test_template_any() {
        let attr = create_test_attribute(
            "test.template",
            AttributeType::Template(TemplateTypeSpec::Any),
            None,
        );
        let kv = get_attribute_name_value(&attr);
        assert_eq!(kv, KeyValue::new("test.template.key", "template_any_value"));
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
    fn test_v2_attribute() {
        use std::collections::BTreeMap;
        use weaver_forge::v2::attribute::Attribute as V2Attribute;
        use weaver_semconv::v2::CommonFields;

        let attr = V2Attribute {
            key: "test.v2.string".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: Some(Examples::String("v2_example".to_owned())),
            common: CommonFields {
                brief: "Test v2 attribute".to_owned(),
                note: String::new(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: BTreeMap::new(),
            },
        };
        let kv = get_attribute_name_value_v2(&attr);
        assert_eq!(kv, KeyValue::new("test.v2.string", "v2_example"));
    }
}
