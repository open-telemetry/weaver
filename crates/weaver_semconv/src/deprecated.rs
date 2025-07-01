// SPDX-License-Identifier: Apache-2.0

//! An enum to represent the different ways to deprecate an attribute, a metric, ...
//!
//! Two formats are supported:
//! - A string with the deprecation message (old format)
//! - A map with the action (renamed or removed) and optionally a note. When the
//!   action is renamed, the map must also contain the field renamed_to.

use schemars::JsonSchema;
use serde::de::{MapAccess, Visitor};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::fmt;
use std::fmt::{Display, Formatter};

/// The different ways to deprecate an attribute, a metric, ...
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "reason")]
pub enum Deprecated {
    /// The telemetry object containing the deprecated field has been renamed to an
    /// existing or a new telemetry object.
    Renamed {
        /// The new name of the telemetry object.
        renamed_to: String,
        /// The note to provide more context about the deprecation.
        note: String,
    },
    /// The telemetry object containing the deprecated field has been obsoleted
    /// because it no longer exists and has no valid replacement.
    ///
    /// The `brief` field should contain the reason why the field has been obsoleted.
    Obsoleted {
        /// The note to provide more context about the deprecation.
        note: String,
    },
    /// The telemetry object containing the deprecated field has been deprecated for
    /// complex reasons (split, merge, ...) which are currently not precisely defined
    /// in the supported deprecation reasons.
    ///
    /// The `brief` field should contain the reason for this uncategorized deprecation.
    Uncategorized {
        /// The note to provide more context about the deprecation.
        note: String,
    },

    /// This variant is used to capture old, unstructured deprecated "string".
    /// Used for backward-compatibility only.
    Unspecified {
        /// The note to provide more context about the deprecation.
        note: String,
    },
}

/// Custom deserialization function to handle both old and new formats.
/// The old format is a string with the deprecation message.
/// The new format is a map with the action (renamed or removed) and optionally a note. When the
/// action is renamed, the map must also contain the field `rename_to`.
pub fn deserialize_deprecated<'de, D>(deserializer: D) -> Result<Deprecated, D::Error>
where
    D: Deserializer<'de>,
{
    // Define the visitor to handle both the old and new formats
    struct DeprecatedVisitor;

    impl<'de> Visitor<'de> for DeprecatedVisitor {
        type Value = Deprecated;

        fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
            formatter.write_str("a string or a map for deprecated field")
        }

        /// Handle the old format (just a string)
        ///
        /// Note: The old format of the deprecated field is a string with the deprecation message.
        /// The new format is a map with at least the `reason` field and the deprecation message is
        /// expected to be in the standard `note` field.
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Deprecated::Unspecified {
                note: value.to_owned(),
            })
        }

        // Handle the new format (a map with action and optionally `rename_to` or `note`)
        fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
        where
            V: MapAccess<'de>,
        {
            let mut action = None;
            let mut new_name = None;
            let mut note = None;

            while let Some(key) = map.next_key::<String>()? {
                match key.as_str() {
                    "reason" => action = Some(map.next_value::<String>()?),
                    "renamed_to" => new_name = Some(map.next_value()?),
                    "note" => note = Some(map.next_value()?),
                    _ => {
                        return Err(de::Error::unknown_field(
                            &key,
                            &["reason", "note", "renamed_to"],
                        ))
                    }
                }
            }

            match action.as_deref() {
                Some("renamed") => {
                    let renamed_to =
                        new_name.ok_or_else(|| de::Error::missing_field("rename_to"))?;
                    let note = note.unwrap_or_else(|| format!("Replaced by `{renamed_to}`."));
                    Ok(Deprecated::Renamed { renamed_to, note })
                }
                Some("obsoleted") => Ok(Deprecated::Obsoleted {
                    note: note.unwrap_or_else(|| "Obsoleted.".to_owned()),
                }),
                Some("uncategorized") => Ok(Deprecated::Uncategorized {
                    note: note.unwrap_or_else(|| "Uncategorized.".to_owned()),
                }),
                _ => Err(de::Error::missing_field("action")),
            }
        }
    }

    deserializer.deserialize_any(DeprecatedVisitor)
}

/// Custom deserialization function to handle both old and new formats for an optional field.
pub fn deserialize_option_deprecated<'de, D>(
    deserializer: D,
) -> Result<Option<Deprecated>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OptionDeprecatedVisitor;

    impl<'de> Visitor<'de> for OptionDeprecatedVisitor {
        type Value = Option<Deprecated>;

        fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
            formatter.write_str("A deprecated field must be either a text string or an object with a reason field combined with associated fields.")
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            // If we encounter an empty value (unit), we return None
            Ok(None)
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            // Explicitly handle the None case (e.g., empty field)
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            // Use the existing deserializer for Deprecated values and wrap the result in Some
            let deprecated = deserialize_deprecated(deserializer)?;
            Ok(Some(deprecated))
        }
    }

    deserializer.deserialize_option(OptionDeprecatedVisitor)
}

/// Implements a human-readable display for the `Deprecated` enum.
impl Display for Deprecated {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let text = match self {
            Deprecated::Renamed { note, .. }
            | Deprecated::Obsoleted { note }
            | Deprecated::Uncategorized { note }
            | Deprecated::Unspecified { note } => note,
        };
        write!(f, "{text}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Item {
        #[serde(deserialize_with = "deserialize_option_deprecated", default)]
        deprecated: Option<Deprecated>,
    }

    #[test]
    fn test_deser_and_to_string() {
        let yaml_data = r#"
- deprecated: 'Replaced by `jvm.buffer.memory.used`.'
- deprecated:
    reason: obsoleted
- deprecated:
    reason: renamed
    renamed_to: foo.unique_id
- deprecated:
    reason: uncategorized
    note: This field is deprecated for some complex reasons.
- deprecated:
    reason: renamed
    renamed_to: foo.unique_id
    note: Replaced by a new attribute `foo.unique_id`.
"#;

        let items: Vec<Item> = serde_yaml::from_str(yaml_data).unwrap();
        assert_eq!(items.len(), 5);
        assert_eq!(
            items[0].deprecated,
            Some(Deprecated::Unspecified {
                note: "Replaced by `jvm.buffer.memory.used`.".to_owned()
            })
        );
        assert_eq!(
            items[1].deprecated,
            Some(Deprecated::Obsoleted {
                note: "Obsoleted.".to_owned()
            })
        );
        assert_eq!(
            items[2].deprecated,
            Some(Deprecated::Renamed {
                renamed_to: "foo.unique_id".to_owned(),
                note: "Replaced by `foo.unique_id`.".to_owned()
            })
        );
        assert_eq!(
            items[3].deprecated,
            Some(Deprecated::Uncategorized {
                note: "This field is deprecated for some complex reasons.".to_owned()
            })
        );
        assert_eq!(
            items[4].deprecated,
            Some(Deprecated::Renamed {
                renamed_to: "foo.unique_id".to_owned(),
                note: "Replaced by a new attribute `foo.unique_id`.".to_owned()
            })
        );
    }
}
