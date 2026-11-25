// SPDX-License-Identifier: Apache-2.0

//! v2 deprecations. Unlike v1, they can't be a plain string.

use std::fmt::{Display, Formatter};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::deprecated::Deprecated as V1Deprecated;

/// Why an attribute, signal or enum member is deprecated.
#[derive(
    Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum Deprecated {
    /// Replaced by `renamed_to`, with the same semantics.
    Renamed {
        /// Name of the replacement.
        renamed_to: String,
        /// Extra context.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(with = "String")]
        note: Option<String>,
    },
    /// Removed, with no replacement.
    Obsoleted {
        /// Why it was removed.
        note: String,
    },
    /// Deprecated for another reason, such as a split or a merge.
    Uncategorized {
        /// What happened and what to use instead.
        note: String,
    },
}

impl Deprecated {
    /// The note, if any. Only `renamed` can omit it.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        match self {
            Deprecated::Renamed { note, .. } => note.as_deref(),
            Deprecated::Obsoleted { note } | Deprecated::Uncategorized { note } => Some(note),
        }
    }
}

/// The note, or `Replaced by `<renamed_to>`.` for a `renamed` without one.
impl Display for Deprecated {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Deprecated::Renamed {
                renamed_to,
                note: None,
            } => write!(f, "Replaced by `{renamed_to}`."),
            Deprecated::Renamed {
                note: Some(note), ..
            }
            | Deprecated::Obsoleted { note }
            | Deprecated::Uncategorized { note } => f.write_str(note),
        }
    }
}

/// v1 plain string deprecations become `uncategorized`.
impl From<V1Deprecated> for Deprecated {
    fn from(d: V1Deprecated) -> Self {
        match d {
            V1Deprecated::Renamed { renamed_to, note } => Deprecated::Renamed { renamed_to, note },
            V1Deprecated::Obsoleted { note } => Deprecated::Obsoleted { note },
            V1Deprecated::Uncategorized { note } | V1Deprecated::Unspecified { note } => {
                Deprecated::Uncategorized { note }
            }
        }
    }
}

impl From<Deprecated> for V1Deprecated {
    fn from(d: Deprecated) -> Self {
        match d {
            Deprecated::Renamed { renamed_to, note } => V1Deprecated::Renamed { renamed_to, note },
            Deprecated::Obsoleted { note } => V1Deprecated::Obsoleted { note },
            Deprecated::Uncategorized { note } => V1Deprecated::Uncategorized { note },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Item {
        #[serde(default)]
        deprecated: Option<Deprecated>,
    }

    #[test]
    fn test_deser() {
        let items: Vec<Item> = serde_saphyr::from_str(
            r#"
- deprecated:
    reason: obsoleted
    note: Removed.
- deprecated:
    reason: renamed
    renamed_to: foo.unique_id
- deprecated:
    reason: uncategorized
    note: Split into `foo` and `bar`.
- {}
"#,
        )
        .unwrap();
        assert_eq!(
            items[0].deprecated,
            Some(Deprecated::Obsoleted {
                note: "Removed.".to_owned()
            })
        );
        assert_eq!(
            items[1].deprecated,
            Some(Deprecated::Renamed {
                renamed_to: "foo.unique_id".to_owned(),
                note: None
            })
        );
        assert_eq!(
            items[2].deprecated,
            Some(Deprecated::Uncategorized {
                note: "Split into `foo` and `bar`.".to_owned()
            })
        );
        assert_eq!(items[3].deprecated, None);
    }

    #[test]
    fn test_deser_rejects_invalid() {
        for yaml in [
            "deprecated: 'Replaced by `foo`.'",
            "deprecated:\n  reason: unspecified\n  note: foo",
            "deprecated:\n  reason: obsoleted\n  note: foo\n  renamed_to: foo",
            "deprecated:\n  reason: uncategorized\n  note: foo\n  renamed_to: foo",
            "deprecated:\n  reason: obsoleted",
            "deprecated:\n  reason: uncategorized",
            "deprecated:\n  reason: renamed",
        ] {
            assert!(
                serde_saphyr::from_str::<Item>(yaml).is_err(),
                "should reject: {yaml}"
            );
        }
    }

    #[test]
    fn test_renamed_without_note() {
        let d = Deprecated::Renamed {
            renamed_to: "foo".to_owned(),
            note: None,
        };
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"reason": "renamed", "renamed_to": "foo"})
        );
        assert_eq!(serde_json::from_value::<Deprecated>(json).unwrap(), d);
        assert_eq!(d.note(), None);
        assert_eq!(d.to_string(), "Replaced by `foo`.");
    }

    #[test]
    fn test_to_v1() {
        assert_eq!(
            V1Deprecated::from(Deprecated::Renamed {
                renamed_to: "foo".to_owned(),
                note: None,
            })
            .to_string(),
            "Replaced by `foo`."
        );
        assert_eq!(
            V1Deprecated::from(Deprecated::Obsoleted {
                note: "Removed.".to_owned()
            }),
            V1Deprecated::Obsoleted {
                note: "Removed.".to_owned()
            }
        );
        assert_eq!(
            V1Deprecated::from(Deprecated::Uncategorized {
                note: "Split.".to_owned()
            }),
            V1Deprecated::Uncategorized {
                note: "Split.".to_owned()
            }
        );
    }

    #[test]
    fn test_from_v1() {
        assert_eq!(
            Deprecated::from(V1Deprecated::Unspecified {
                note: "Use `foo`.".to_owned()
            }),
            Deprecated::Uncategorized {
                note: "Use `foo`.".to_owned()
            }
        );
    }
}
