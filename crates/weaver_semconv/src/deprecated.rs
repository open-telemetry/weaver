// SPDX-License-Identifier: Apache-2.0

//! An enum to represent the different ways to deprecate an attribute, a metric, ...
//!
//! Two formats are supported:
//! - A string with the deprecation message (old format)
//! - A map with the action (renamed or removed) and optionally a note. When the
//!   action is renamed, the map must also contain the field renamed_to.

use regex::Regex;
use schemars::JsonSchema;
use serde::de::{MapAccess, Visitor};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::fmt;
use std::fmt::{Display, Formatter};

/// The different ways to deprecate an attribute, a metric, ...
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "action")]
pub enum Deprecated {
    /// The object containing the deprecated field has been renamed to an
    /// existing object or to a new object.
    Renamed {
        /// The new name of the field.
        new_name: String,
        /// An optional note to explain why the field has been renamed.
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// The object containing the deprecated field has been deprecated
    /// either because it no longer exists, has been split into multiple fields,
    /// has been renamed in various ways across different contexts, or for any other reason.
    Deprecated {
        /// A note to explain why the field has been deprecated.
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

        // Handle the old format (just a string)
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            // Regex to match "Replaced by `some_field`"
            // ToDo make this regex global
            // Replaced by `([\w]+(?:\.[\w]+)*)`?
            let renamed_regex =
                Regex::new(r"(?i)(?:replace[d]? by|use|use the) `([\w]+(?:\.[\w]+)*)`?")
                    .map_err(E::custom)?;

            if let Some(captures) = renamed_regex.captures(value) {
                // This is the old format for renamed fields
                let rename_to = captures.get(1).map_or("", |m| m.as_str()).to_owned();
                Ok(Deprecated::Renamed {
                    new_name: rename_to,
                    note: Some(value.to_owned()),
                })
            } else {
                Ok(Deprecated::Deprecated {
                    note: value.to_owned(),
                })
            }
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
                    "action" => action = Some(map.next_value::<String>()?),
                    "new_name" => new_name = Some(map.next_value()?),
                    "note" => note = Some(map.next_value()?),
                    _ => {
                        return Err(de::Error::unknown_field(
                            &key,
                            &["action", "new_name", "note"],
                        ))
                    }
                }
            }

            match action.as_deref() {
                Some("renamed") => {
                    let rename_to =
                        new_name.ok_or_else(|| de::Error::missing_field("rename_to"))?;
                    Ok(Deprecated::Renamed {
                        new_name: rename_to,
                        note,
                    })
                }
                Some("deprecated") => {
                    let note = note.ok_or_else(|| de::Error::missing_field("note"))?;
                    Ok(Deprecated::Deprecated { note })
                }
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
            formatter.write_str("a string, a map, or nothing for a deprecated field")
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
        match self {
            Deprecated::Renamed {
                new_name: rename_to,
                note,
            } => {
                if let Some(note) = note.as_ref() {
                    write!(f, "{}", note)
                } else {
                    write!(f, "Replaced by `{}`.", rename_to)
                }
            }
            Deprecated::Deprecated { note } => {
                write!(f, "{}", note)
            }
        }
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
    action: deprecated
    note: This attribute is no longer used.
- deprecated: 
    action: deprecated
    note: Should no longer be used.
- deprecated:
    action: renamed
    new_name: foo.unique_id
    note: This field has been renamed for consistency.
- deprecated:
    action: renamed
    new_name: foo.unique_id
- deprecated: Removed.
- deprecated: Replaced by `gen_ai.usage.input_tokens` attribute.
- deprecated:
- deprecated: 'Replaced by `server.address` on client spans and `client.address` on server spans.'
- deprecated: 'Split to `network.transport` and `network.type`.'
- deprecated: "Replaced by `db.client.connection.state`."
- deprecated: "Replaced by `db.client.connection.state."
"#;

        let items: Vec<Item> = serde_yaml::from_str(yaml_data).unwrap();
        assert_eq!(items.len(), 12);
        assert_eq!(
            items[0].deprecated.clone().unwrap().to_string(),
            "Replaced by `jvm.buffer.memory.used`.".to_owned()
        );
        assert_eq!(
            items[0].deprecated,
            Some(Deprecated::Renamed {
                new_name: "jvm.buffer.memory.used".to_owned(),
                note: Some("Replaced by `jvm.buffer.memory.used`.".to_owned())
            })
        );
        assert_eq!(
            items[1].deprecated.clone().unwrap().to_string(),
            "This attribute is no longer used.".to_owned()
        );
        assert_eq!(
            items[1].deprecated,
            Some(Deprecated::Deprecated {
                note: "This attribute is no longer used.".to_owned()
            })
        );
        assert_eq!(
            items[2].deprecated.clone().unwrap().to_string(),
            "Should no longer be used.".to_owned()
        );
        assert_eq!(
            items[2].deprecated,
            Some(Deprecated::Deprecated {
                note: "Should no longer be used.".to_owned()
            })
        );
        assert_eq!(
            items[3].deprecated.clone().unwrap().to_string(),
            "This field has been renamed for consistency.".to_owned()
        );
        assert_eq!(
            items[3].deprecated,
            Some(Deprecated::Renamed {
                new_name: "foo.unique_id".to_owned(),
                note: Some("This field has been renamed for consistency.".to_owned())
            })
        );
        assert_eq!(
            items[4].deprecated.clone().unwrap().to_string(),
            "Replaced by `foo.unique_id`.".to_owned()
        );
        assert_eq!(
            items[4].deprecated,
            Some(Deprecated::Renamed {
                new_name: "foo.unique_id".to_owned(),
                note: None
            })
        );
        assert_eq!(
            items[5].deprecated.clone().unwrap().to_string(),
            "Removed.".to_owned()
        );
        assert_eq!(
            items[5].deprecated,
            Some(Deprecated::Deprecated {
                note: "Removed.".to_owned()
            })
        );
        assert_eq!(
            items[6].deprecated.clone().unwrap().to_string(),
            "Replaced by `gen_ai.usage.input_tokens` attribute.".to_owned()
        );
        assert_eq!(
            items[6].deprecated,
            Some(Deprecated::Renamed {
                new_name: "gen_ai.usage.input_tokens".to_owned(),
                note: Some("Replaced by `gen_ai.usage.input_tokens` attribute.".to_owned())
            })
        );
        assert_eq!(items[7].deprecated, None);
        assert_eq!(
            items[8].deprecated.clone().unwrap().to_string(),
            "Replaced by `server.address` on client spans and `client.address` on server spans."
                .to_owned()
        );
        assert_eq!(items[8].deprecated, Some(Deprecated::Renamed {
            new_name: "server.address".to_owned(),
            note: Some("Replaced by `server.address` on client spans and `client.address` on server spans.".to_owned())
        }));
        assert_eq!(
            items[9].deprecated.clone().unwrap().to_string(),
            "Split to `network.transport` and `network.type`.".to_owned()
        );
        assert_eq!(
            items[9].deprecated,
            Some(Deprecated::Deprecated {
                note: "Split to `network.transport` and `network.type`.".to_owned()
            })
        );
        assert_eq!(
            items[10].deprecated,
            Some(Deprecated::Renamed {
                new_name: "db.client.connection.state".to_owned(),
                note: Some("Replaced by `db.client.connection.state`.".to_owned())
            })
        );
        assert_eq!(
            items[11].deprecated,
            Some(Deprecated::Renamed {
                new_name: "db.client.connection.state".to_owned(),
                note: Some("Replaced by `db.client.connection.state.".to_owned())
            })
        );
    }
}
