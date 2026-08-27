//! Entity related definition structs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    signal_requirement_level::SignalRequirementLevel,
    v2::{
        attribute::RequirementLevel,
        signal_id::SignalId,
        CommonFields,
    },
};

use crate::v2::{attribute::AttributeRef, provenance::Provenance, Signal};

/// The definition of an Entity signal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct Entity {
    /// The type of the Entity.
    pub r#type: SignalId,

    /// The attributes that make the identity of the Entity.
    pub identity: Vec<EntityAttributeRef>,
    /// The attributes that make the description of the Entity.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub description: Vec<EntityAttributeRef>,

    /// The requirement level of the entity. Defaults to 'recommended' when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<SignalRequirementLevel>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,

    /// The provenance of the Entity.
    #[serde(default)]
    #[serde(skip_serializing_if = "Provenance::is_empty")]
    pub provenance: Provenance,
}

/// A reference to an entity definition, from an `entity_associations` clause.
///
/// The reference is self-describing: it names the entity, and it names the
/// registry that owns the definition. A consumer that holds one therefore knows
/// which schema to read next.
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

/// An entity association expression in a resolved schema.
///
/// The shape matches the authored expression. Only the leaf differs: an author
/// writes an entity type, and a resolved schema holds an [`EntityRef`], which
/// also says where that entity is defined.
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

/// Turns resolved association expressions back into the authored form, where a
/// leaf is a name alone.
///
/// A v1 group and a markdown table both read a name, so every consumer of the
/// two needs this.
#[must_use]
pub fn to_named_associations(
    associations: &[EntityAssociation],
) -> Vec<weaver_semconv::entity_association::EntityAssociation> {
    use weaver_semconv::entity_association::EntityAssociation as SpecAssociation;
    associations
        .iter()
        .map(|assoc| match assoc {
            EntityAssociation::Ref(entity_ref) => {
                SpecAssociation::Ref(entity_ref.r#type.to_string())
            }
            EntityAssociation::OneOf { one_of } => SpecAssociation::OneOf {
                one_of: to_named_associations(one_of),
            },
            EntityAssociation::AllOf { all_of } => SpecAssociation::AllOf {
                all_of: to_named_associations(all_of),
            },
        })
        .collect()
}

/// A special type of reference to attributes that remembers entity-specicific information.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct EntityAttributeRef {
    /// Reference, by index, to the attribute catalog.
    pub base: AttributeRef,
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as `condition` MUST
    /// specify the conditions under which the attribute is required.
    pub requirement_level: RequirementLevel,
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

impl Signal for Entity {
    fn id(&self) -> &str {
        &self.r#type
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::provenance::DependencyRef;

    /// A tree with a local leaf and a leaf from a dependency.
    fn association_tree() -> Vec<EntityAssociation> {
        vec![EntityAssociation::AllOf {
            all_of: vec![
                EntityAssociation::Ref(EntityRef::local("service".to_owned().into())),
                EntityAssociation::OneOf {
                    one_of: vec![EntityAssociation::Ref(EntityRef {
                        r#type: "host".to_owned().into(),
                        provenance: Provenance {
                            source: Some(DependencyRef(2)),
                            path: String::new(),
                        },
                    })],
                },
            ],
        }]
    }

    /// A local reference writes the type alone. A reference into a dependency
    /// writes the dependency index beside it. Both read back unchanged.
    #[test]
    fn test_association_round_trips() {
        let associations = association_tree();
        let json = serde_json::to_string(&associations).expect("serialize");
        assert_eq!(
            json,
            r#"[{"all_of":[{"type":"service"},{"one_of":[{"type":"host","provenance":{"source":2}}]}]}]"#
        );
        let reparsed: Vec<EntityAssociation> = serde_json::from_str(&json).expect("parse");
        assert_eq!(reparsed, associations);
    }

    /// The old leaf was a bare name. A reader that accepted it would take a
    /// name for a reference and lose the registry it points at.
    #[test]
    fn test_bare_name_leaf_is_refused() {
        let result: Result<Vec<EntityAssociation>, _> = serde_json::from_str(r#"["service"]"#);
        assert!(result.is_err(), "a bare name is not an entity reference");
    }

    #[test]
    fn test_refs_walks_the_whole_tree() {
        let associations = association_tree();
        let types: Vec<&str> = associations
            .iter()
            .flat_map(EntityAssociation::refs)
            .map(|r| r.r#type.as_ref())
            .collect();
        assert_eq!(types.len(), 2);
        assert!(types.contains(&"service"));
        assert!(types.contains(&"host"));
    }

    /// The authored form keeps the shape and drops the provenance.
    #[test]
    fn test_to_named_associations_keeps_the_shape() {
        use weaver_semconv::entity_association::EntityAssociation as SpecAssociation;
        assert_eq!(
            to_named_associations(&association_tree()),
            vec![SpecAssociation::AllOf {
                all_of: vec![
                    SpecAssociation::Ref("service".to_owned()),
                    SpecAssociation::OneOf {
                        one_of: vec![SpecAssociation::Ref("host".to_owned())],
                    },
                ],
            }]
        );
    }
}
