// Intermediary format for telemetry samples

use serde::{Deserialize, Serialize};
use serde_json::Value;
use weaver_semconv::attribute::PrimitiveOrArrayTypeSpec;

/// Represents a sample telemetry attribute parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleAttribute {
    /// The name of the attribute
    pub name: String,
    /// The value of the attribute
    pub value: Option<Value>,
    /// The type of the attribute's value
    /// This may be available in the upstream source, an o11y vendor for example
    pub r#type: Option<PrimitiveOrArrayTypeSpec>,
}

impl SampleAttribute {
    /// Infer the type of the attribute from the value
    pub fn infer_type(&mut self) {
        if self.r#type.is_none() {
            self.r#type = match &self.value {
                Some(value) => match value {
                    Value::Null => None,
                    Value::Bool(_) => Some(PrimitiveOrArrayTypeSpec::Boolean),
                    Value::Number(_) => {
                        // Int or double?
                        if value.is_i64() || value.is_u64() {
                            Some(PrimitiveOrArrayTypeSpec::Int)
                        } else {
                            Some(PrimitiveOrArrayTypeSpec::Double)
                        }
                    }
                    Value::String(_) => Some(PrimitiveOrArrayTypeSpec::String),
                    Value::Array(_) => {
                        // Strings, Ints, Doubles or Booleans?
                        // Get the first non-null element to check types
                        if let Some(arr) = value.as_array() {
                            let first_value = arr.iter().find(|v| !v.is_null());

                            match first_value {
                                Some(first) => {
                                    // Check if all elements match the type of the first element
                                    if first.is_boolean()
                                        && arr.iter().all(|v| v.is_null() || v.is_boolean())
                                    {
                                        Some(PrimitiveOrArrayTypeSpec::Booleans)
                                    } else if (first.is_i64() || first.is_u64())
                                        && arr
                                            .iter()
                                            .all(|v| v.is_null() || v.is_i64() || v.is_u64())
                                    {
                                        Some(PrimitiveOrArrayTypeSpec::Ints)
                                    } else if first.is_number()
                                        && arr.iter().all(|v| v.is_null() || v.is_number())
                                    {
                                        Some(PrimitiveOrArrayTypeSpec::Doubles)
                                    } else if first.is_string()
                                        && arr.iter().all(|v| v.is_null() || v.is_string())
                                    {
                                        Some(PrimitiveOrArrayTypeSpec::Strings)
                                    } else {
                                        // Mixed or unsupported types
                                        None
                                    }
                                }
                                None => None, // Empty or all-null array
                            }
                        } else {
                            None
                        }
                    }
                    Value::Object(_) => None, // Unsupported
                },
                None => None,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_infer_null_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!(null)),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, None);
    }

    #[test]
    fn test_infer_boolean_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!(true)),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::Boolean));
    }

    #[test]
    fn test_infer_int_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!(42)),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::Int));
    }

    #[test]
    fn test_infer_double_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!(3.15)),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::Double));
    }

    #[test]
    fn test_infer_string_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!("hello")),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::String));
    }

    #[test]
    fn test_infer_booleans_array_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!([true, false, null])),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::Booleans));
    }

    #[test]
    fn test_infer_ints_array_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!([1, 2, null, 3])),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::Ints));
    }

    #[test]
    fn test_infer_doubles_array_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!([1.1, 2.2, null, 3.0])),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::Doubles));
    }

    #[test]
    fn test_infer_strings_array_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!(["a", "b", null, "c"])),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::Strings));
    }

    #[test]
    fn test_infer_mixed_array_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!([1, "string", true])),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, None);
    }

    #[test]
    fn test_infer_empty_array_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!([])),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, None);
    }

    #[test]
    fn test_infer_all_null_array_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!([null, null])),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, None);
    }

    #[test]
    fn test_infer_object_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!({"key": "value"})),
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, None);
    }

    #[test]
    fn test_no_value() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: None,
            r#type: None,
        };
        attr.infer_type();
        assert_eq!(attr.r#type, None);
    }

    #[test]
    fn test_preserve_existing_type() {
        let mut attr = SampleAttribute {
            name: "test".to_owned(),
            value: Some(json!("string")),
            r#type: Some(PrimitiveOrArrayTypeSpec::Int), // Deliberately wrong type
        };
        attr.infer_type();
        assert_eq!(attr.r#type, Some(PrimitiveOrArrayTypeSpec::Int)); // Should not change
    }
}
