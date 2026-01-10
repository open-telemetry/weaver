// SPDX-License-Identifier: Apache-2.0

//! A custom OrderedF64 implementation that supports schemars v1.

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

/// A wrapper around f64 that provides total ordering and hashing.
///
/// Treats NaN values as equal to each other and greater than all other values.
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct OrderedF64(pub f64);

impl Deref for OrderedF64 {
    type Target = f64;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for OrderedF64 {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Debug for OrderedF64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for OrderedF64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl PartialEq for OrderedF64 {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        if self.0.is_nan() && other.0.is_nan() {
            true
        } else {
            self.0 == other.0
        }
    }
}

impl Eq for OrderedF64 {}

impl PartialOrd for OrderedF64 {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedF64 {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.0.partial_cmp(&other.0) {
            Some(ordering) => ordering,
            None => {
                if self.0.is_nan() {
                    if other.0.is_nan() {
                        Ordering::Equal
                    } else {
                        Ordering::Greater
                    }
                } else {
                    Ordering::Less
                }
            }
        }
    }
}

impl Hash for OrderedF64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let bits = if self.0.is_nan() {
            f64::NAN.to_bits()
        } else if self.0 == 0.0 {
            0u64
        } else {
            self.0.to_bits()
        };
        bits.hash(state);
    }
}

impl From<f64> for OrderedF64 {
    #[inline]
    fn from(val: f64) -> Self {
        OrderedF64(val)
    }
}

impl From<OrderedF64> for f64 {
    #[inline]
    fn from(val: OrderedF64) -> Self {
        val.0
    }
}

impl JsonSchema for OrderedF64 {
    fn schema_name() -> Cow<'static, str> {
        "double".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::OrderedF64").into()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "number",
            "format": "double"
        })
    }

    fn inline_schema() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    #[test]
    fn test_ordering() {
        assert!(OrderedF64(1.0) < OrderedF64(2.0));
        assert!(OrderedF64(2.0) > OrderedF64(1.0));
        assert!(OrderedF64(1.0) == OrderedF64(1.0));
    }

    #[test]
    fn test_nan_ordering() {
        let nan = OrderedF64(f64::NAN);
        let one = OrderedF64(1.0);
        assert!(nan > one);
        assert!(nan == nan);
    }

    #[test]
    fn test_hash_consistency() {
        fn hash<T: Hash>(value: &T) -> u64 {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        }

        assert_eq!(hash(&OrderedF64(1.0)), hash(&OrderedF64(1.0)));
        assert_eq!(hash(&OrderedF64(0.0)), hash(&OrderedF64(-0.0)));
        assert_eq!(hash(&OrderedF64(f64::NAN)), hash(&OrderedF64(f64::NAN)));
    }

    #[test]
    fn test_serialization() {
        let value = OrderedF64(3.15);
        let serialized = serde_json::to_string(&value).unwrap();
        assert_eq!(serialized, "3.15");

        let deserialized: OrderedF64 = serde_json::from_str("3.15").unwrap();
        assert_eq!(deserialized, value);
    }

    #[test]
    fn test_json_schema() {
        let schema = schemars::schema_for!(OrderedF64);
        let schema_str = serde_json::to_string_pretty(&schema).unwrap();
        assert!(schema_str.contains("\"type\": \"number\""));
    }
}
