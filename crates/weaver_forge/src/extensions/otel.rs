// SPDX-License-Identifier: Apache-2.0

//! Set of filters, tests, and functions that are specific to the OpenTelemetry project.

use crate::config::CaseConvention;
use minijinja::{ErrorKind, Value};

/// Converts registry.{namespace}.{other}.{components} to {namespace}.
///
/// A [`minijinja::Error`] is returned if the input does not start with "registry" or does not have
/// at least two parts. Otherwise, it returns the namespace (second part of the input).
pub(crate) fn attribute_registry_namespace(input: &str) -> Result<String, minijinja::Error> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() < 2 || parts[0] != "registry" {
        return Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("This attribute registry id `{}` is invalid", input),
        ));
    }
    Ok(parts[1].to_owned())
}

/// Converts registry.{namespace}.{other}.{components} to {Namespace} (title case the namespace).
///
/// A [`minijinja::Error`] is returned if the input does not start with "registry" or does not have
/// at least two parts. Otherwise, it returns the namespace (second part of the input, title case).
pub(crate) fn attribute_registry_title(input: &str) -> Result<String, minijinja::Error> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() < 2 || parts[0] != "registry" {
        return Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("This attribute registry id `{}` is invalid", input),
        ));
    }
    Ok(CaseConvention::TitleCase.convert(parts[1]))
}

/// attribute_registry_file: Converts registry.{namespace}.{other}.{components} to attributes-registry/{namespace}.md (kebab-case namespace).
///
/// A [`minijinja::Error`] is returned if the input does not start with "registry" or does not have
/// at least two parts. Otherwise, it returns the file path (kebab-case namespace).
pub(crate) fn attribute_registry_file(input: &str) -> Result<String, minijinja::Error> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() < 2 || parts[0] != "registry" {
        return Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("This attribute registry id `{}` is invalid", input),
        ));
    }
    Ok(format!(
        "attributes-registry/{}.md",
        CaseConvention::KebabCase.convert(parts[1])
    ))
}

/// Converts metric.{namespace}.{other}.{components} to {namespace}.
///
/// A [`minijinja::Error`] is returned if the input does not start with "metric" or does not have
/// at least two parts. Otherwise, it returns the namespace (second part of the input).
pub(crate) fn metric_namespace(input: &str) -> Result<String, minijinja::Error> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() < 2 || parts[0] != "metric" {
        return Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("This metric id `{}` is invalid", input),
        ));
    }
    Ok(parts[1].to_owned())
}

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
    use crate::extensions::otel::{
        attribute_registry_file, attribute_registry_namespace, attribute_registry_title,
        is_deprecated, is_experimental, is_stable, metric_namespace,
    };
    use minijinja::value::StructObject;
    use minijinja::Value;

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
    fn test_attribute_registry_namespace() {
        // A string that does not start with "registry"
        let input = "test";
        assert!(attribute_registry_namespace(input).is_err());

        // A string that starts with "registry" but does not have at least two parts
        let input = "registry";
        assert!(attribute_registry_namespace(input).is_err());

        // A string that starts with "registry" and has at least two parts
        let input = "registry.namespace.other.components";
        assert_eq!(attribute_registry_namespace(input).unwrap(), "namespace");

        // An empty string
        let input = "";
        assert!(attribute_registry_namespace(input).is_err());
    }

    #[test]
    fn test_attribute_registry_title() {
        // A string that does not start with "registry"
        let input = "test";
        assert!(attribute_registry_title(input).is_err());

        // A string that starts with "registry" but does not have at least two parts
        let input = "registry";
        assert!(attribute_registry_title(input).is_err());

        // A string that starts with "registry" and has at least two parts
        let input = "registry.namespace.other.components";
        assert_eq!(attribute_registry_title(input).unwrap(), "Namespace");

        // An empty string
        let input = "";
        assert!(attribute_registry_title(input).is_err());
    }

    #[test]
    fn test_attribute_registry_file() {
        // A string that does not start with "registry"
        let input = "test";
        assert!(attribute_registry_file(input).is_err());

        // A string that starts with "registry" but does not have at least two parts
        let input = "registry";
        assert!(attribute_registry_file(input).is_err());

        // A string that starts with "registry" and has at least two parts
        let input = "registry.namespace.other.components";
        assert_eq!(
            attribute_registry_file(input).unwrap(),
            "attributes-registry/namespace.md"
        );

        // An empty string
        let input = "";
        assert!(attribute_registry_file(input).is_err());
    }

    #[test]
    fn test_metric_namespace() {
        // A string that does not start with "registry"
        let input = "test";
        assert!(metric_namespace(input).is_err());

        // A string that starts with "registry" but does not have at least two parts
        let input = "metric";
        assert!(metric_namespace(input).is_err());

        // A string that starts with "registry" and has at least two parts
        let input = "metric.namespace.other.components";
        assert_eq!(metric_namespace(input).unwrap(), "namespace");

        // An empty string
        let input = "";
        assert!(metric_namespace(input).is_err());
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
