//! Event related definition structs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::v2::{
    attribute::RequirementLevel, signal_id::SignalId,
    signal_requirement_level::SignalRequirementLevel, CommonFields,
};

use crate::v2::{
    attribute::AttributeRef, entity::EntityAssociation, provenance::Provenance, Signal,
};

/// The definition of an Event signal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    /// The list is an implicit `one_of` (telemetry must satisfy at least one entry); each entry is an
    /// entity reference (a type plus its provenance) or a nested `one_of`/`all_of` expression.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<EntityAssociation>,

    /// The requirement level of the event. Defaults to 'recommended' when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<SignalRequirementLevel>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,

    /// The provenance of the Event.
    #[serde(default)]
    #[serde(skip_serializing_if = "Provenance::is_empty")]
    pub provenance: Provenance,
}

/// A special type of reference to attributes that remembers event-specicific information.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct EventAttributeRef {
    /// Reference, by index, to the attribute catalog.
    pub base: AttributeRef,
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as `condition` MUST
    /// specify the conditions under which the attribute is required.
    pub requirement_level: RequirementLevel,
}

/// A refinement of an event, for use in code-gen or specific library application.
///
/// A refinement represents a "view" of a Event that is highly optimised for a particular implementation.
/// e.g. for HTTP events, there may be a refinement that provides only the necessary information for dealing with Java's HTTP
/// client library, and drops optional or extraneous information from the underlying http event.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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

impl Signal for Event {
    fn id(&self) -> &str {
        &self.name
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_signal() {
        let event = Event {
            name: SignalId::from("exception"),
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: CommonFields {
                brief: "Exception event".to_owned(),
                note: "".to_owned(),
                stability: Default::default(),
                deprecated: None,
                annotations: Default::default(),
            },
            provenance: Default::default(),
        };

        assert_eq!(event.id(), "exception");
        assert_eq!(event.common().brief, "Exception event");
    }
}
