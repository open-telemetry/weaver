// SPDX-License-Identifier: Apache-2.0

//! This crate provides bare minimum support for colorized string differencing.

use serde_json::Value;
use similar::TextDiff;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

/// Constructs a "diff" string of the original vs. updated.
/// Will create colorized (ANSI) output w/ `+` representing additions and `-` representing removals.
#[must_use]
pub fn diff_output(original: &str, updated: &str) -> String {
    let mut result = String::new();
    let diff = TextDiff::from_lines(original, updated);
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            similar::ChangeTag::Delete => "-",
            similar::ChangeTag::Insert => "+",
            similar::ChangeTag::Equal => " ",
        };
        let color = match change.tag() {
            similar::ChangeTag::Delete => RED,
            similar::ChangeTag::Insert => GREEN,
            similar::ChangeTag::Equal => RESET,
        };
        result.push_str(&format!("{color}{sign} {change}"));
    }
    result.push_str(RESET);
    result
}

/// Displays differences between two directories and returns whether they are identical.
/// The function will print differences to stderr.
#[allow(clippy::print_stderr)]
pub fn diff_dir<P: AsRef<Path>>(expected_dir: P, observed_dir: P) -> std::io::Result<bool> {
    let mut expected_files = HashSet::new();
    let mut observed_files = HashSet::new();

    // Walk through the first directory and add files to files1 set
    for entry in WalkDir::new(&expected_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            let relative_path = path
                .strip_prefix(&expected_dir)
                .map_err(std::io::Error::other)?;
            _ = expected_files.insert(relative_path.to_path_buf());
        }
    }

    // Walk through the second directory and add files to files2 set
    for entry in WalkDir::new(&observed_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            let relative_path = path
                .strip_prefix(&observed_dir)
                .map_err(std::io::Error::other)?;
            _ = observed_files.insert(relative_path.to_path_buf());
        }
    }

    // Assume directories are identical until proven otherwise
    let mut are_identical = true;

    // Compare files in both sets
    for file in expected_files.intersection(&observed_files) {
        let file1_content =
            fs::read_to_string(expected_dir.as_ref().join(file))?.replace("\r\n", "\n");
        let file2_content =
            fs::read_to_string(observed_dir.as_ref().join(file))?.replace("\r\n", "\n");

        if file1_content != file2_content {
            are_identical = false;
            eprintln!(
                "Files {:?} and {:?} are different",
                expected_dir.as_ref().join(file),
                observed_dir.as_ref().join(file)
            );

            eprintln!(
                "Found differences:\n{}",
                diff_output(&file1_content, &file2_content)
            );
            break;
        }
    }
    // If any file is unique to one directory, they are not identical
    let not_in_observed = expected_files
        .difference(&observed_files)
        .collect::<Vec<_>>();
    if !not_in_observed.is_empty() {
        are_identical = false;
        eprintln!("Observed output is missing files: {not_in_observed:?}");
    }
    let not_in_expected = observed_files
        .difference(&expected_files)
        .collect::<Vec<_>>();
    if !not_in_expected.is_empty() {
        are_identical = false;
        eprintln!("Observed output has unexpected files: {not_in_expected:?}");
    }

    Ok(are_identical)
}

#[macro_export]
/// Macro to simplify comparing two strings with a diff.
macro_rules! assert_string_eq {
    ($lhs:expr, $rhs:expr) => {
        assert_eq!(
            $lhs,
            $rhs,
            "Found string differences: {}",
            weaver_diff::diff_output($lhs, $rhs)
        );
    };
}

/// Canonicalizes a JSON string by parsing it into a `serde_json::Value` and then canonicalizing it.
///
/// # Returns
/// - The canonicalized and prettyfied JSON string.
pub fn canonicalize_json_string(value: &str) -> Result<String, serde_json::error::Error> {
    let json_value: Value = serde_json::from_str(value)?;
    let canonicalized = canonicalize_json(json_value);
    serde_json::to_string_pretty(&canonicalized)
}

/// Recursively canonicalizes a JSON value by sorting objects and arrays.
/// - Objects: Keys are sorted lexicographically (handled automatically by `BTreeMap`).
/// - Arrays:
///   - Arrays of primitive values are sorted based on their values.
///   - Arrays containing objects or arrays are sorted based on their canonical form.
/// - Primitive Values: Returned as is.
///
/// # Returns
/// - A new `serde_json::Value` that is the canonical form of the input value.
pub fn canonicalize_json(value: Value) -> Value {
    match value {
        // Primitive types are returned as is.
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value,
        Value::Array(arr) => {
            // Recursively canonicalize each item in the array.
            let mut sorted_items: Vec<Value> = arr.into_iter().map(canonicalize_json).collect();
            // Sort the array using `compare_values` to ensure consistent ordering.
            sorted_items.sort_by(compare_values);
            Value::Array(sorted_items)
        }
        Value::Object(map) => {
            // Recursively canonicalize each value in the object.
            let sorted_map = map
                .into_iter()
                .map(|(k, v)| (k, canonicalize_json(v)))
                .collect::<serde_json::Map<String, Value>>();
            Value::Object(sorted_map)
            // No need to sort keys; BTreeMap keeps them sorted.
        }
    }
}

/// Compares two `serde_json::Value` instances for sorting purposes.
/// Defines a total ordering among JSON values to allow sorting within arrays.
/// The order is defined as:
/// `Null < Bool < Number < String < Array < Object`
///
/// # Returns
/// - An `Ordering` indicating the relative order of `a` and `b`.
fn compare_values(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        // Both values are `Null`.
        (Value::Null, Value::Null) => Ordering::Equal,
        // `Null` is less than any other type.
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,

        // Both values are booleans.
        (Value::Bool(a_bool), Value::Bool(b_bool)) => a_bool.cmp(b_bool),
        // `Bool` is less than `Number`, `String`, `Array`, and `Object`.
        (Value::Bool(_), _) => Ordering::Less,
        (_, Value::Bool(_)) => Ordering::Greater,

        // Both values are numbers.
        // Compare numbers as floating-point values.
        (Value::Number(a_num), Value::Number(b_num)) => a_num
            .as_f64()
            .partial_cmp(&b_num.as_f64())
            .unwrap_or(Ordering::Equal), // Handle NaN cases gracefully.
        // `Number` is less than `String`, `Array`, and `Object`.
        (Value::Number(_), _) => Ordering::Less,
        (_, Value::Number(_)) => Ordering::Greater,

        // Both values are strings.
        (Value::String(a_str), Value::String(b_str)) => a_str.cmp(b_str),
        // `String` is less than `Array` and `Object`.
        (Value::String(_), _) => Ordering::Less,
        (_, Value::String(_)) => Ordering::Greater,

        // Both values are arrays.
        (Value::Array(a_arr), Value::Array(b_arr)) => compare_arrays(a_arr, b_arr),
        // `Array` is less than `Object`.
        (Value::Array(_), _) => Ordering::Less,
        (_, Value::Array(_)) => Ordering::Greater,

        // Both values are objects.
        (Value::Object(a_obj), Value::Object(b_obj)) => compare_objects(a_obj, b_obj),
    }
}

/// Compares two JSON arrays element-wise for sorting purposes.
/// Arrays are compared based on their elements:
/// - First, by length.
/// - Then, by comparing each pair of elements in order.
///
/// # Returns
/// - An `Ordering` indicating the relative order of `a` and `b`.
fn compare_arrays(a: &[Value], b: &[Value]) -> Ordering {
    // Compare lengths first.
    let len_cmp = a.len().cmp(&b.len());
    if len_cmp != Ordering::Equal {
        return len_cmp;
    }

    // Compare each pair of elements.
    for (a_item, b_item) in a.iter().zip(b.iter()) {
        let ord = compare_values(a_item, b_item);
        if ord != Ordering::Equal {
            return ord;
        }
    }

    // All elements are equal.
    Ordering::Equal
}

/// Compares two JSON objects by their sorted key-value pairs for sorting purposes.
/// Objects are compared based on:
/// - Their number of key-value pairs.
/// - The keys and associated values in lexicographical order.
///
/// # Returns
/// - An `Ordering` indicating the relative order of `a` and `b`.
fn compare_objects(
    a: &serde_json::Map<String, Value>,
    b: &serde_json::Map<String, Value>,
) -> Ordering {
    // Compare number of key-value pairs.
    let len_cmp = a.len().cmp(&b.len());
    if len_cmp != Ordering::Equal {
        return len_cmp;
    }

    // Iterate over key-value pairs (keys are sorted in BTreeMap).
    let mut a_iter = a.iter();
    let mut b_iter = b.iter();

    loop {
        match (a_iter.next(), b_iter.next()) {
            (Some((a_key, a_val)), Some((b_key, b_val))) => {
                // Compare keys.
                let key_cmp = a_key.cmp(b_key);
                if key_cmp != Ordering::Equal {
                    return key_cmp;
                }
                // Compare associated values.
                let val_cmp = compare_values(a_val, b_val);
                if val_cmp != Ordering::Equal {
                    return val_cmp;
                }
            }
            // Both iterators have reached the end.
            (None, None) => break,
            // This case should not occur since lengths are equal.
            _ => unreachable!("Maps have the same length; this should not happen."),
        }
    }

    // All key-value pairs are equal.
    Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_diff_output() {
        let original = "Hello, world!";
        let updated = "Hello, there!";
        let diff = diff_output(original, updated);
        assert_eq!(
            diff,
            "\u{1b}[31m- Hello, world!\n\u{1b}[32m+ Hello, there!\n\u{1b}[0m"
        );
    }

    #[test]
    fn test_diff_dir() {
        let expected_dir = "./src";
        let observed_dir = "./src";

        let are_identical =
            diff_dir(&expected_dir, &observed_dir).expect("Failed to diff directories");
        assert!(are_identical);

        let expected_dir = "./src";
        let observed_dir = "../weaver_cache/src";

        let are_identical =
            diff_dir(&expected_dir, &observed_dir).expect("Failed to diff directories");
        assert!(!are_identical);
    }

    #[test]
    fn test_canonicalize_primitives() {
        let null = Value::Null;
        assert_eq!(canonicalize_json(null.clone()), null);

        let boolean = Value::Bool(true);
        assert_eq!(canonicalize_json(boolean.clone()), boolean);

        let number = Value::Number(serde_json::Number::from(42));
        assert_eq!(canonicalize_json(number.clone()), number);

        let string = Value::String(String::from("hello"));
        assert_eq!(canonicalize_json(string.clone()), string);
    }

    #[test]
    fn test_canonicalize_empty_array_and_object() {
        let empty_array = json!([]);
        assert_eq!(canonicalize_json(empty_array.clone()), empty_array);

        let empty_object = json!({});
        assert_eq!(canonicalize_json(empty_object.clone()), empty_object);
    }

    #[test]
    fn test_canonicalize_array_of_primitives() {
        let array = json!([3, 1, 2]);
        let expected = json!([1, 2, 3]);
        assert_eq!(canonicalize_json(array), expected);
    }

    #[test]
    fn test_canonicalize_array_of_arrays() {
        let array = json!([[3, 1, 2], [6, 5, 4]]);
        let expected = json!([[1, 2, 3], [4, 5, 6]]);
        assert_eq!(canonicalize_json(array), expected);
    }

    #[test]
    fn test_canonicalize_array_of_objects() {
        let array = json!([
            {"b": 2, "a": 1},
            {"d": 4, "c": 3}
        ]);
        let expected = json!([
            {"a": 1, "b": 2},
            {"c": 3, "d": 4}
        ]);
        assert_eq!(canonicalize_json(array), expected);
    }

    #[test]
    fn test_canonicalize_nested_structures() {
        let json_value = json!({
            "b": [3, 1, 2],
            "a": {"y": 2, "x": 1},
            "c": [{"b": 2}, {"a": 1}]
        });
        let expected = json!({
            "a": {
                "x": 1,
                "y": 2
            },
            "b": [
                1,
                2,
                3
            ],
            "c": [
                {"a": 1},
                {"b": 2}
            ]
        });
        assert_eq!(canonicalize_json(json_value), expected);
    }

    #[test]
    fn test_canonicalize_numbers() {
        let array = json!([3.1, 2.2, 1.3]);
        let expected = json!([1.3, 2.2, 3.1]);
        assert_eq!(canonicalize_json(array), expected);
    }

    #[test]
    fn test_canonicalize_mixed_types_in_array() {
        let array = json!([null, "string", 2, true, 3]);
        let expected = json!([null, true, 2, 3, "string"]);
        assert_eq!(canonicalize_json(array.clone()), expected);
    }

    #[test]
    fn test_canonicalize_mixed_array() {
        let array = json!([{"a": 2}, [3, 1], "b", 4]);
        let expected = json!([4, "b", [1, 3], {"a": 2}]);
        assert_eq!(canonicalize_json(array), expected);
    }

    #[test]
    fn test_canonicalize_complex() {
        let json_value = json!({
            "z": [{"b": [3, 2, 1]}, {"a": [6, 5, 4]}],
            "y": {"b": {"d": 4, "c": 3}, "a": {"b": 2, "a": 1}},
            "x": [9, 7, 8]
        });
        let expected = json!({
            "x": [7, 8, 9],
            "y": {
                "a": {"a": 1, "b": 2},
                "b": {"c": 3, "d": 4}
            },
            "z": [
                {"a": [4, 5, 6]},
                {"b": [1, 2, 3]}
            ]
        });
        assert_eq!(canonicalize_json(json_value), expected);
    }

    #[test]
    fn test_compare_values() {
        let a = json!(1);
        let b = json!(2);
        assert_eq!(compare_values(&a, &b), Ordering::Less);

        let a = json!("apple");
        let b = json!("banana");
        assert_eq!(compare_values(&a, &b), Ordering::Less);

        let a = json!([1, 2, 3]);
        let b = json!([1, 2, 4]);
        assert_eq!(compare_values(&a, &b), Ordering::Less);

        let a = json!({"a": 1});
        let b = json!({"a": 2});
        assert_eq!(compare_values(&a, &b), Ordering::Less);
    }

    #[test]
    fn test_handling_of_special_numbers() {
        let array = json!([0.0, -0.0, 1.0, -1.0]);
        let expected = json!([-1.0, -0.0, 0.0, 1.0]);
        assert_eq!(canonicalize_json(array), expected);
    }

    #[test]
    fn test_unicode_strings() {
        let array = json!(["éclair", "apple", "Æther", "zebra"]);
        let expected = json!(["apple", "zebra", "Æther", "éclair"]);
        assert_eq!(canonicalize_json(array), expected);
    }
}
