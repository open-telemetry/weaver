use serde_yaml::Value;
use std::collections::BTreeMap;
use weaver_semconv::YamlValue;

/// Merges 'override_map' into 'base_map', key by key.
///
/// This allows annotatoins to be extended "piecemeal" in refinements, e.g.
///
/// We allow `x.y.z = "a"` and `x.y.v = "b"` to merge such that the result is
/// `x.y = { z = "a", v = "b" }`.
pub(crate) fn merge_annotations(
    base_map: &mut BTreeMap<String, YamlValue>,
    override_map: &BTreeMap<String, YamlValue>,
) {
    for (k, v) in override_map {
        if let Some(base_v) = base_map.get_mut(k) {
            merge_yaml_values(&mut base_v.0, &v.0);
        } else {
            _ = base_map.insert(k.clone(), v.clone());
        }
    }
}

/// Recursively merges 'override_val' into 'base'.
/// - If both are mappings (objects), it merges key by key.
/// - If they are lists or primitives, 'override_val' replaces 'base'.
fn merge_yaml_values(base: &mut Value, override_val: &Value) {
    match (base, override_val) {
        (Value::Mapping(base_map), Value::Mapping(override_map)) => {
            for (k, v) in override_map {
                if let Some(base_v) = base_map.get_mut(k) {
                    merge_yaml_values(base_v, v);
                } else {
                    _ = base_map.insert(k.clone(), v.clone());
                }
            }
        }
        (b, o) => *b = o.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::{Mapping, Value};

    #[test]
    fn test_merge_yaml_values_primitives() {
        let mut base = Value::String("base".to_string());
        let override_val = Value::Number(42.into());
        merge_yaml_values(&mut base, &override_val);
        assert_eq!(base, Value::Number(42.into()));
    }

    #[test]
    fn test_merge_yaml_values_arrays() {
        let mut base = Value::Sequence(vec![Value::Number(1.into())]);
        let override_val = Value::Sequence(vec![Value::Number(2.into()), Value::Number(3.into())]);
        merge_yaml_values(&mut base, &override_val);
        assert_eq!(
            base,
            Value::Sequence(vec![Value::Number(2.into()), Value::Number(3.into())])
        );
    }

    #[test]
    fn test_merge_yaml_values_mappings() {
        let mut base_map = Mapping::new();
        _ = base_map.insert(Value::String("a".to_string()), Value::Number(1.into()));
        let mut base = Value::Mapping(base_map);

        let mut override_map = Mapping::new();
        _ = override_map.insert(Value::String("b".to_string()), Value::Number(2.into()));
        let override_val = Value::Mapping(override_map);

        merge_yaml_values(&mut base, &override_val);

        let mut expected_map = Mapping::new();
        _ = expected_map.insert(Value::String("a".to_string()), Value::Number(1.into()));
        _ = expected_map.insert(Value::String("b".to_string()), Value::Number(2.into()));
        assert_eq!(base, Value::Mapping(expected_map));
    }

    #[test]
    fn test_merge_yaml_values_nested_mappings() {
        // Base: { a: { b: 1, c: 2 }, d: 3 }
        let mut base_nested = Mapping::new();
        _ = base_nested.insert(Value::String("b".to_string()), Value::Number(1.into()));
        _ = base_nested.insert(Value::String("c".to_string()), Value::Number(2.into()));
        let mut base_map = Mapping::new();
        _ = base_map.insert(Value::String("a".to_string()), Value::Mapping(base_nested));
        _ = base_map.insert(Value::String("d".to_string()), Value::Number(3.into()));
        let mut base = Value::Mapping(base_map);

        // Override: { a: { c: 99, e: 4 } }
        let mut override_nested = Mapping::new();
        _ = override_nested.insert(Value::String("c".to_string()), Value::Number(99.into()));
        _ = override_nested.insert(Value::String("e".to_string()), Value::Number(4.into()));
        let mut override_map = Mapping::new();
        _ = override_map.insert(
            Value::String("a".to_string()),
            Value::Mapping(override_nested),
        );
        let override_val = Value::Mapping(override_map);

        merge_yaml_values(&mut base, &override_val);

        // Expected: { a: { b: 1, c: 99, e: 4 }, d: 3 }
        let mut expected_nested = Mapping::new();
        _ = expected_nested.insert(Value::String("b".to_string()), Value::Number(1.into()));
        _ = expected_nested.insert(Value::String("c".to_string()), Value::Number(99.into()));
        _ = expected_nested.insert(Value::String("e".to_string()), Value::Number(4.into()));
        let mut expected_map = Mapping::new();
        _ = expected_map.insert(Value::String("a".to_string()), Value::Mapping(expected_nested));
        _ = expected_map.insert(Value::String("d".to_string()), Value::Number(3.into()));
        assert_eq!(base, Value::Mapping(expected_map));
    }
}
