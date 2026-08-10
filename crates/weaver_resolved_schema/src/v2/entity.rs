//! Entity related definition structs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    attribute::RequirementLevel,
    signal_requirement_level::SignalRequirementLevel,
    v2::{signal_id::SignalId, CommonFields},
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

/// A reference, by index, to an entity refinement of the schema.
#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct EntityRef(pub u32);

impl std::fmt::Display for EntityRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EntityRef({})", self.0)
    }
}

/// An entity association expression in a resolved schema.
///
/// The leaf is an index into the entity refinements, not a name. Every entity
/// definition also has a base refinement, so an index can reach a definition or
/// a refinement of one.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum EntityAssociation {
    /// A reference to one entity refinement.
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
    pub fn refs(&self) -> impl Iterator<Item = EntityRef> + '_ {
        // A small explicit stack keeps this allocation-light and avoids
        // recursion in a hot path.
        let mut stack = vec![self];
        std::iter::from_fn(move || {
            while let Some(node) = stack.pop() {
                match node {
                    EntityAssociation::Ref(r) => return Some(*r),
                    EntityAssociation::OneOf { one_of: children }
                    | EntityAssociation::AllOf { all_of: children } => stack.extend(children),
                }
            }
            None
        })
    }
}

/// Turns the indices of association expressions back into names, using the
/// entity refinements of the schema. Returns the first index that names no
/// refinement.
///
/// A resolved schema holds an index. A v1 group and a template both read a
/// name, so every consumer of the two needs this.
pub fn to_named_associations(
    associations: &[EntityAssociation],
    refinements: &[EntityRefinement],
) -> Result<Vec<weaver_semconv::entity_association::EntityAssociation>, EntityRef> {
    use weaver_semconv::entity_association::EntityAssociation as SpecAssociation;
    associations
        .iter()
        .map(|assoc| match assoc {
            EntityAssociation::Ref(entity_ref) => refinements
                .get(entity_ref.0 as usize)
                .map(|refinement| SpecAssociation::Ref(refinement.id.to_string()))
                .ok_or(*entity_ref),
            EntityAssociation::OneOf { one_of } => Ok(SpecAssociation::OneOf {
                one_of: to_named_associations(one_of, refinements)?,
            }),
            EntityAssociation::AllOf { all_of } => Ok(SpecAssociation::AllOf {
                all_of: to_named_associations(all_of, refinements)?,
            }),
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
