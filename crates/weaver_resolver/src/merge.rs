use serde_json::Value;
use std::collections::BTreeMap;
use weaver_semconv::YamlValue;

/// Merges 'override_map' into 'base_map', key by key.
///
/// This allows annotatoins to be extended "piecemeal" in refinements, e.g.
///
/// We allow `x.y.z = "a"` and `x.y.v = "b"` to merge such that the result is
/// `x.y = { z = "a", v = "b" }`.
pub(crate) fn merge_annotations(
    mut base_map: BTreeMap<String, YamlValue>,
    override_map: &BTreeMap<String, YamlValue>,
) -> BTreeMap<String, YamlValue> {
    for (k, v) in override_map {
        if let Some(base_v) = base_map.get_mut(k) {
            let old_val = std::mem::take(&mut base_v.0);
            base_v.0 = merge_yaml_values(old_val, &v.0);
        } else {
            _ = base_map.insert(k.clone(), v.clone());
        }
    }
    base_map
}

/// Recursively merges 'override_val' into 'base'.
/// - If both are mappings (objects), it merges key by key.
/// - If they are lists or primitives, 'override_val' replaces 'base'.
fn merge_yaml_values(base: Value, override_val: &Value) -> Value {
    match (base, override_val) {
        (Value::Object(mut base_map), Value::Object(override_map)) => {
            for (k, v) in override_map {
                if let Some(base_v) = base_map.get_mut(k) {
                    let old_val = std::mem::take(base_v);
                    *base_v = merge_yaml_values(old_val, v);
                } else {
                    _ = base_map.insert(k.clone(), v.clone());
                }
            }
            Value::Object(base_map)
        }
        (_, o) => o.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn test_merge_yaml_values_primitives() {
        let base = Value::String("base".to_owned());
        let override_val = Value::Number(42.into());
        let base = merge_yaml_values(base, &override_val);
        assert_eq!(base, Value::Number(42.into()));
    }

    #[test]
    fn test_merge_yaml_values_arrays() {
        let base = json!([1]);
        let override_val = json!([2, 3]);
        let base = merge_yaml_values(base, &override_val);
        assert_eq!(base, json!([2, 3]));
    }

    #[test]
    fn test_merge_yaml_values_mappings() {
        let base = json!({"a": 1});
        let override_val = json!({"b": 2});
        let base = merge_yaml_values(base, &override_val);
        assert_eq!(base, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn test_merge_yaml_values_nested_mappings() {
        // Base: { a: { b: 1, c: 2 }, d: 3 }
        let base = json!({
            "a": { "b": 1, "c": 2 },
            "d": 3
        });

        // Override: { a: { c: 99, e: 4 } }
        let override_val = json!({
            "a": { "c": 99, "e": 4 }
        });

        let base = merge_yaml_values(base, &override_val);

        // Expected: { a: { b: 1, c: 99, e: 4 }, d: 3 }
        let expected = json!({
            "a": { "b": 1, "c": 99, "e": 4 },
            "d": 3
        });
        assert_eq!(base, expected);
    }
}
