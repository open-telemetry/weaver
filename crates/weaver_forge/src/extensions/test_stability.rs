// SPDX-License-Identifier: Apache-2.0

//! Set of tests

use minijinja::Value;

/// Checks if the input value is an object with a field named "stability" that has the value "stable".
/// Otherwise, it returns false.
#[must_use]
pub(crate) fn is_stable(input: Value) -> bool {
    let result = input.get_attr("stability");

    if let Ok(stability) = result {
        if let Some(stability) = stability.as_str() {
            return stability == "stable";
        }
    }
    false
}

/// Checks if the input value is an object with a field named "stability" that has the value
/// "experimental". Otherwise, it returns false.
#[must_use]
pub(crate) fn is_experimental(input: Value) -> bool {
    let result = input.get_attr("stability");

    if let Ok(stability) = result {
        if let Some(stability) = stability.as_str() {
            return stability == "experimental";
        }
    }
    false
}

/// Checks if the input value is an object with a field named "stability" that has the value "deprecated".
/// Otherwise, it returns false.
#[must_use]
pub(crate) fn is_deprecated(input: Value) -> bool {
    let result = input.get_attr("deprecated");

    if let Ok(deprecated) = result {
        if let Some(deprecated) = deprecated.as_str() {
            return !deprecated.is_empty();
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use minijinja::value::StructObject;

    use super::*;

    struct DynAttr {
        id: String,
        r#type: String,
        stability: String,
        deprecated: Option<String>,
    }

    impl StructObject for DynAttr {
        fn get_field(&self, field: &str) -> Option<Value> {
            match field {
                "id" => Some(Value::from(self.id.as_str())),
                "type" => Some(Value::from(self.r#type.as_str())),
                "stability" => Some(Value::from(self.stability.as_str())),
                "deprecated" => self.deprecated.as_ref().map(|s| Value::from(s.as_str())),
                _ => None,
            }
        }
    }

    struct DynSomethingElse {
        id: String,
        r#type: String,
    }

    impl StructObject for DynSomethingElse {
        fn get_field(&self, field: &str) -> Option<Value> {
            match field {
                "id" => Some(Value::from(self.id.as_str())),
                "type" => Some(Value::from(self.r#type.as_str())),
                _ => None,
            }
        }
    }

    #[test]
    fn test_is_stable() {
        // An attribute with stability "stable"
        let attr = Value::from_struct_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "stable".to_owned(),
            deprecated: None,
        });
        assert!(is_stable(attr));

        // An attribute with stability "deprecated"
        let attr = Value::from_struct_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "deprecated".to_owned(),
            deprecated: None,
        });
        assert!(!is_stable(attr));

        // An object without a stability field
        let object = Value::from_struct_object(DynSomethingElse {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
        });
        assert!(!is_stable(object));
    }

    #[test]
    fn test_is_experimental() {
        // An attribute with stability "experimental"
        let attr = Value::from_struct_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "experimental".to_owned(),
            deprecated: None,
        });
        assert!(is_experimental(attr));

        // An attribute with stability "stable"
        let attr = Value::from_struct_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "stable".to_owned(),
            deprecated: None,
        });
        assert!(!is_experimental(attr));

        // An object without a stability field
        let object = Value::from_struct_object(DynSomethingElse {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
        });
        assert!(!is_experimental(object));
    }

    #[test]
    fn test_is_deprecated() {
        // An attribute with stability "experimental" and a deprecated field with a value
        let attr = Value::from_struct_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "experimental".to_owned(),
            deprecated: Some("This is deprecated".to_owned()),
        });
        assert!(is_deprecated(attr));

        // An attribute with stability "stable" and a deprecated field with a value
        let attr = Value::from_struct_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "stable".to_owned(),
            deprecated: Some("This is deprecated".to_owned()),
        });
        assert!(is_deprecated(attr));

        // An object without a deprecated field
        let object = Value::from_struct_object(DynSomethingElse {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
        });
        assert!(!is_deprecated(object));

        let attr = Value::from_struct_object(DynAttr {
            id: "test".to_owned(),
            r#type: "test".to_owned(),
            stability: "stable".to_owned(),
            deprecated: None,
        });
        assert!(!is_deprecated(attr));
    }
}
