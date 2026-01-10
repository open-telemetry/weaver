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
    fn test_deref() {
        assert_eq!(*OrderedF64(2.5), 2.5);
    }

    #[test]
    fn test_deref_mut() {
        let mut value = OrderedF64(2.5);
        *value = 3.5;
        assert_eq!(*value, 3.5);
    }

    #[test]
    fn test_debug() {
        assert_eq!(format!("{:?}", OrderedF64(2.5)), "2.5");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", OrderedF64(2.5)), "2.5");
    }

    #[test]
    fn test_partial_eq() {
        assert!(OrderedF64(1.0) == OrderedF64(1.0));
        assert!(OrderedF64(1.0) != OrderedF64(2.0));
        assert!(OrderedF64(f64::NAN) == OrderedF64(f64::NAN));
    }

    #[test]
    fn test_partial_ord() {
        assert_eq!(OrderedF64(1.0).partial_cmp(&OrderedF64(2.0)), Some(Ordering::Less));
    }

    #[test]
    fn test_ord() {
        assert_eq!(OrderedF64(1.0).cmp(&OrderedF64(2.0)), Ordering::Less);
        assert_eq!(OrderedF64(f64::NAN).cmp(&OrderedF64(1.0)), Ordering::Greater);
        assert_eq!(OrderedF64(1.0).cmp(&OrderedF64(f64::NAN)), Ordering::Less);
        assert_eq!(OrderedF64(f64::NAN).cmp(&OrderedF64(f64::NAN)), Ordering::Equal);
    }

    #[test]
    fn test_hash() {
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
    fn test_from_f64() {
        let value: OrderedF64 = 2.5.into();
        assert_eq!(*value, 2.5);
    }

    #[test]
    fn test_into_f64() {
        let f: f64 = OrderedF64(2.5).into();
        assert_eq!(f, 2.5);
    }

    #[test]
    fn test_json_schema() {
        let schema = schemars::schema_for!(OrderedF64);
        let schema_str = serde_json::to_string_pretty(&schema).unwrap();
        assert!(schema_str.contains("\"type\": \"number\""));
    }

    #[test]
    fn test_default() {
        assert_eq!(*OrderedF64::default(), 0.0);
    }

    #[test]
    fn test_clone() {
        let a = OrderedF64(2.5);
        assert_eq!(a.clone(), a);
    }

    #[test]
    fn test_serialize() {
        assert_eq!(serde_json::to_string(&OrderedF64(3.15)).unwrap(), "3.15");
    }

    #[test]
    fn test_deserialize() {
        let value: OrderedF64 = serde_json::from_str("3.15").unwrap();
        assert_eq!(value, OrderedF64(3.15));
    }
}
