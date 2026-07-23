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
