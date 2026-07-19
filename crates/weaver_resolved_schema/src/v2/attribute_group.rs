// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define attribute groups going forward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    attribute::RequirementLevel,
    v2::{signal_id::SignalId, CommonFields},
};

use crate::v2::{attribute::AttributeRef, provenance::Provenance, Signal};

/// Public attribute group.
///
/// An attribute group is a grouping of attributes that can be leveraged
/// in codegen. For example, rather than passing attributes on at a time,
/// a temporary structure could be made to contain all of them and report
/// the bundle as a group to different signals.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct AttributeGroup {
    /// The name of the attribute group, must be unique.
    pub id: SignalId,

    /// List of attributes and group references that belong to this group
    pub attributes: Vec<AttributeGroupAttributeRef>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,

    /// The provenance of the AttributeGroup.
    #[serde(default)]
    #[serde(skip_serializing_if = "Provenance::is_empty")]
    pub provenance: Provenance,
}

/// A reference to an attribute in a public attribute group that remembers the
/// group-specific requirement level refinement.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct AttributeGroupAttributeRef {
    /// Reference, by index, to the attribute catalog.
    pub base: AttributeRef,
    /// The requirement level of the attribute within this group. One of
    /// "required", "conditionally_required", "recommended" or "opt_in".
    /// When set to "conditionally_required", the string provided as
    /// `condition` MUST specify the conditions under which the attribute
    /// is required.
    pub requirement_level: RequirementLevel,
}

impl Signal for AttributeGroup {
    fn id(&self) -> &str {
        &self.id
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
}
