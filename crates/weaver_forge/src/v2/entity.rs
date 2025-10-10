//! Event related definitions structs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    attribute::RequirementLevel,
    v2::{signal_id::SignalId, CommonFields},
};

use crate::v2::attribute::Attribute;

/// The definition of an event signal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    /// The type of the entity.
    pub r#type: SignalId,

    /// List of attributes that belong to this event.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub identity: Vec<EntityAttribute>,

    /// List of attributes that belong to this event.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub description: Vec<EntityAttribute>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A special type of reference to attributes that remembers event-specicific information.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityAttribute {
    /// Base attribute definitions.
    #[serde(flatten)]
    pub base: Attribute,
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as <condition> MUST
    /// specify the conditions under which the attribute is required.
    ///
    /// Note: For attributes that are "recommended" or "opt-in" - not all metric source will
    /// create timeseries with these attributes, but for any given timeseries instance, the attributes that *were* present
    /// should *remain* present. That is - a metric timeseries cannot drop attributes during its lifetime.
    pub requirement_level: RequirementLevel,
}
