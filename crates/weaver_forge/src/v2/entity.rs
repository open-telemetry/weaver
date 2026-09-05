//! Event related definitions structs.

use crate::v2::provenance::Provenance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    schema_url::SchemaUrl,
    v2::{
        attribute::RequirementLevel, signal_id::SignalId,
        signal_requirement_level::SignalRequirementLevel, CommonFields,
    },
};

use crate::v2::attribute::Attribute;

/// The definition of an entity signal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct Entity {
    /// The type of the entity.
    pub r#type: SignalId,

    /// List of attributes that identify this entity.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub identity: Vec<EntityAttribute>,

    /// List of attributes that describe this entity.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub description: Vec<EntityAttribute>,

    /// The requirement level of the entity. Defaults to 'recommended' when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<SignalRequirementLevel>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
    /// The provenance of the entity.
    #[serde(default)]
    #[serde(skip_serializing_if = "Provenance::is_empty")]
    pub provenance: Provenance,
}

/// A reference to an entity definition, from an `entity_associations` clause.
///
/// The reference names the entity, and names the registry that defines it, so a
/// consumer that holds one knows where to read the definition. Pass it to
/// [`crate::v2::registry::ForgeResolvedRegistry::lookup_entity`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct EntityRef {
    /// The entity type, or the id of an entity refinement.
    pub r#type: SignalId,

    /// Where the entity is defined. Empty means this registry.
    #[serde(default)]
    #[serde(skip_serializing_if = "Provenance::is_empty")]
    pub provenance: Provenance,
}

impl EntityRef {
    /// Returns a reference to an entity that this registry defines.
    #[must_use]
    pub fn local(r#type: SignalId) -> Self {
        EntityRef {
            r#type,
            provenance: Provenance::default(),
        }
    }
}

/// An entity association expression.
///
/// The shape matches the authored expression. Only the leaf differs: an author
/// writes an entity type, and this view holds an [`EntityRef`], which also says
/// where that entity is defined.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum EntityAssociation {
    /// A reference to one entity.
    Ref(EntityRef),
    /// Satisfied when at least one of the contained expressions is satisfied.
    OneOf {
        /// The candidate expressions.
        // `no_recursion` stops utoipa from inlining the self-referential schema
        // forever (it would otherwise overflow the stack at generation time).
        #[cfg_attr(feature = "openapi", schema(no_recursion))]
        one_of: Vec<EntityAssociation>,
    },
    /// Satisfied when every contained expression is satisfied.
    AllOf {
        /// The required expressions.
        #[cfg_attr(feature = "openapi", schema(no_recursion))]
        all_of: Vec<EntityAssociation>,
    },
}

impl EntityAssociation {
    /// Returns every entity reference anywhere in this expression tree.
    pub fn refs(&self) -> impl Iterator<Item = &EntityRef> {
        // A small explicit stack keeps this allocation-light and avoids
        // recursion in a hot path.
        let mut stack = vec![self];
        std::iter::from_fn(move || {
            while let Some(node) = stack.pop() {
                match node {
                    EntityAssociation::Ref(r) => return Some(r),
                    EntityAssociation::OneOf { one_of: children }
                    | EntityAssociation::AllOf { all_of: children } => stack.extend(children),
                }
            }
            None
        })
    }
}

/// Turns resolved association expressions into materialized ones, which name
/// the defining registry by schema url rather than by index.
///
/// `deps` is the dependency list of the schema the expressions came from.
#[must_use]
pub fn from_resolved_associations(
    associations: &[weaver_resolved_schema::v2::entity::EntityAssociation],
    deps: &[SchemaUrl],
) -> Vec<EntityAssociation> {
    use weaver_resolved_schema::v2::entity::EntityAssociation as ResolvedAssociation;
    associations
        .iter()
        .map(|assoc| match assoc {
            ResolvedAssociation::Ref(entity_ref) => EntityAssociation::Ref(EntityRef {
                r#type: entity_ref.r#type.clone(),
                provenance: Provenance::from_resolved(&entity_ref.provenance, deps),
            }),
            ResolvedAssociation::OneOf { one_of } => EntityAssociation::OneOf {
                one_of: from_resolved_associations(one_of, deps),
            },
            ResolvedAssociation::AllOf { all_of } => EntityAssociation::AllOf {
                all_of: from_resolved_associations(all_of, deps),
            },
        })
        .collect()
}

/// A refinement of an entity signal.
///
/// Describes an entity optimized for a specific environment,
/// for example, a host entity might be refined for a specific OS
/// and describe how base entity attributes are obtained in that OS.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct EntityRefinement {
    /// The identity of the refinement.
    pub id: SignalId,

    /// The definition of the entity refinement.
    #[serde(flatten)]
    pub entity: Entity,
}

/// A special type of reference to attributes that remembers entity-specific information.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct EntityAttribute {
    /// Base attribute definitions.
    #[serde(flatten)]
    pub base: Attribute,
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as `condition`` MUST
    /// specify the conditions under which the attribute is required.
    pub requirement_level: RequirementLevel,
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_resolved_schema::v2::{
        entity::{EntityAssociation as ResolvedAssociation, EntityRef as ResolvedRef},
        provenance::{DependencyRef, Provenance as ResolvedProvenance},
    };

    fn dependency_list() -> Vec<SchemaUrl> {
        vec![
            "https://example.com/base/1.0.0"
                .try_into()
                .expect("a valid schema url"),
            "https://example.com/middle/1.0.0"
                .try_into()
                .expect("a valid schema url"),
        ]
    }

    /// The shape of the expression does not change. A local leaf stays local,
    /// and a leaf into a dependency trades the index for the schema url.
    #[test]
    fn test_from_resolved_associations_names_the_registry() {
        let resolved = vec![ResolvedAssociation::AllOf {
            all_of: vec![
                ResolvedAssociation::Ref(ResolvedRef::local("service".to_owned().into())),
                ResolvedAssociation::OneOf {
                    one_of: vec![ResolvedAssociation::Ref(ResolvedRef {
                        r#type: "host".to_owned().into(),
                        provenance: ResolvedProvenance {
                            source: Some(DependencyRef(1)),
                            path: String::new(),
                        },
                    })],
                },
            ],
        }];
        let deps = dependency_list();
        assert_eq!(
            from_resolved_associations(&resolved, &deps),
            vec![EntityAssociation::AllOf {
                all_of: vec![
                    EntityAssociation::Ref(EntityRef::local("service".to_owned().into())),
                    EntityAssociation::OneOf {
                        one_of: vec![EntityAssociation::Ref(EntityRef {
                            r#type: "host".to_owned().into(),
                            provenance: Provenance {
                                source: Some(deps[1].clone()),
                                path: None,
                            },
                        })],
                    },
                ],
            }]
        );
    }

    /// A local leaf writes the type alone, and a leaf into a dependency writes
    /// the url beside it. Both read back unchanged.
    #[test]
    fn test_association_round_trips() {
        let deps = dependency_list();
        let associations = vec![
            EntityAssociation::Ref(EntityRef::local("service".to_owned().into())),
            EntityAssociation::Ref(EntityRef {
                r#type: "host".to_owned().into(),
                provenance: Provenance {
                    source: Some(deps[0].clone()),
                    path: None,
                },
            }),
        ];
        let json = serde_json::to_string(&associations).expect("serialize");
        assert_eq!(
            json,
            r#"[{"type":"service"},{"type":"host","provenance":{"source":"https://example.com/base/1.0.0"}}]"#
        );
        let reparsed: Vec<EntityAssociation> = serde_json::from_str(&json).expect("parse");
        assert_eq!(reparsed, associations);
    }

    #[test]
    fn test_refs_walks_the_whole_tree() {
        let deps = dependency_list();
        let resolved = vec![ResolvedAssociation::OneOf {
            one_of: vec![
                ResolvedAssociation::Ref(ResolvedRef::local("service".to_owned().into())),
                ResolvedAssociation::Ref(ResolvedRef::local("host".to_owned().into())),
            ],
        }];
        let associations = from_resolved_associations(&resolved, &deps);
        let types: Vec<&str> = associations
            .iter()
            .flat_map(EntityAssociation::refs)
            .map(|r| r.r#type.as_ref())
            .collect();
        assert_eq!(types.len(), 2);
        assert!(types.contains(&"service"));
        assert!(types.contains(&"host"));
    }
}
