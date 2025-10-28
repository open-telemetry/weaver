// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define attribute groups going forward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::v2::{signal_id::SignalId, CommonFields};

use crate::v2::attribute::AttributeRef;

/// Public attribute group.
///
/// An attribute group is a grouping of attributes that can be leveraged
/// in codegen. For example, rather than passing attributes on at a time,
/// a temporary structure could be made to contain all of them and report
/// the bundle as a group to different signals.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct AttributeGroup {
    /// The name of the attribute group, must be unique.
    pub id: SignalId,

    /// List of attributes and group references that belong to this group
    pub attributes: Vec<AttributeOrGroupRef>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A reference to either an attribute or an attribute group.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum AttributeOrGroupRef {
    /// Reference to an attribute.
    Attribute(AttributeRef),
    /// Reference to an attribute group.
    Group(GroupRef),
}

/// A reference to an attribute group.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct GroupRef {
    /// Reference an existing attribute group by id.
    pub ref_group: SignalId,
}
