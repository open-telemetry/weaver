//! Version two of attribute groups.

use crate::v2::provenance::Provenance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    attribute::RequirementLevel,
    v2::{signal_id::SignalId, CommonFields},
};

use crate::v2::attribute::Attribute;

/// Public attribute group.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttributeGroup {
    /// The name of the attribute group, must be unique.
    pub id: SignalId,
    /// List of attributes.
    pub attributes: Vec<AttributeGroupAttribute>,
    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
    /// The provenance of the attribute group.
    #[serde(default)]
    #[serde(skip_serializing_if = "Provenance::is_empty")]
    pub provenance: Provenance,
}

/// An attribute belonging to a public attribute group, carrying the
/// group-specific requirement level.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct AttributeGroupAttribute {
    /// Base attribute definition.
    #[serde(flatten)]
    pub base: Attribute,
    /// The requirement level of the attribute within this group. One of
    /// "required", "conditionally_required", "recommended" or "opt_in".
    /// When set to "conditionally_required", the string provided as
    /// `condition` MUST specify the conditions under which the attribute
    /// is required.
    pub requirement_level: RequirementLevel,
}
