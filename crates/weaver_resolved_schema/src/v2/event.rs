//! Event related definition structs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    attribute::RequirementLevel,
    v2::{signal_id::SignalId, CommonFields},
};

use crate::v2::attribute::AttributeRef;

/// The definition of an Event signal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// The name of the event.
    pub name: SignalId,

    /// List of attributes that belong to this event.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<EventAttributeRef>,

    // TODO - Should Entity Associations be "strong" links?
    /// Which entities this event should be associated with.
    ///
    /// This list is an "any of" list, where a event may be associated with one or more entities, but should
    /// be associated with at least one in this list.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<String>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A special type of reference to attributes that remembers event-specicific information.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventAttributeRef {
    /// Reference, by index, to the attribute catalog.
    pub base: AttributeRef,
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as <condition> MUST
    /// specify the conditions under which the attribute is required.
    pub requirement_level: RequirementLevel,
}

/// A refinement of an event, for use in code-gen or specific library application.
///
/// A refinement represents a "view" of a Event that is highly optimised for a particular implementation.
/// e.g. for HTTP events, there may be a refinement that provides only the necessary information for dealing with Java's HTTP
/// client library, and drops optional or extraneous information from the underlying http event.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct EventRefinement {
    /// The identity of the refinement
    pub id: SignalId,

    // TODO - This is a lazy way of doing this.  We use `name` to refer
    // to the underlying event definition, but override all fields here.
    // We probably should copy-paste all the "event" attributes here
    // including the `ty`
    /// The definition of the event refinement.
    #[serde(flatten)]
    pub event: Event,
}
