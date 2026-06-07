// SPDX-License-Identifier: Apache-2.0

//! Entity association expressions.
//!
//! An entity association declares which entities a signal (span, metric or event) should be
//! associated with. The expression tree supports a reference to a single entity, plus `one_of`
//! and `all_of` combinators that can be nested arbitrarily.
//!
//! A bare list of entity references (the historical syntax) is interpreted as an implicit
//! `one_of`: the telemetry must satisfy at least one of the listed entities.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An entity association expression.
///
/// In YAML this is either a bare string (an entity reference), a `{ one_of: [...] }` map or a
/// `{ all_of: [...] }` map, where each list element is itself an [`EntityAssociation`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum EntityAssociation {
    /// A reference to an entity by its type name.
    Ref(String),
    /// Satisfied when at least one of the contained expressions is satisfied.
    OneOf {
        /// The candidate expressions.
        one_of: Vec<EntityAssociation>,
    },
    /// Satisfied when every contained expression is satisfied.
    AllOf {
        /// The required expressions.
        all_of: Vec<EntityAssociation>,
    },
}

impl EntityAssociation {
    /// Returns an iterator over every entity name referenced anywhere in this expression tree.
    pub fn referenced_entities(&self) -> impl Iterator<Item = &str> {
        // A small explicit stack keeps this allocation-light and avoids recursion in a hot path.
        let mut stack = vec![self];
        std::iter::from_fn(move || {
            while let Some(node) = stack.pop() {
                match node {
                    EntityAssociation::Ref(name) => return Some(name.as_str()),
                    EntityAssociation::OneOf { one_of: children }
                    | EntityAssociation::AllOf { all_of: children } => stack.extend(children),
                }
            }
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bare_string_parses_as_ref() {
        let assoc: EntityAssociation = serde_yaml::from_str("service").expect("parse");
        assert_eq!(assoc, EntityAssociation::Ref("service".to_owned()));
    }

    #[test]
    fn test_nested_one_of_all_of() {
        let yaml = r#"
all_of:
  - service
  - deployment
  - cloud
  - one_of: [host, container]
  - one_of:
      - all_of: [x, y]
      - z
"#;
        let assoc: EntityAssociation = serde_yaml::from_str(yaml).expect("parse");
        let EntityAssociation::AllOf { all_of } = &assoc else {
            panic!("expected all_of, got {assoc:?}");
        };
        assert_eq!(all_of.len(), 5);
        assert_eq!(all_of[0], EntityAssociation::Ref("service".to_owned()));
        assert_eq!(
            all_of[3],
            EntityAssociation::OneOf {
                one_of: vec![
                    EntityAssociation::Ref("host".to_owned()),
                    EntityAssociation::Ref("container".to_owned()),
                ],
            }
        );
        // Round-trips.
        let reparsed: EntityAssociation =
            serde_yaml::from_str(&serde_yaml::to_string(&assoc).expect("serialize"))
                .expect("parse");
        assert_eq!(reparsed, assoc);
    }

    #[test]
    fn test_referenced_entities() {
        let yaml = r#"
all_of:
  - deployment
  - one_of:
      - all_of: [x, y]
      - z
"#;
        let assoc: EntityAssociation = serde_yaml::from_str(yaml).expect("parse");
        let mut names: Vec<_> = assoc.referenced_entities().collect();
        names.sort_unstable();
        assert_eq!(names, vec!["deployment", "x", "y", "z"]);
    }
}
