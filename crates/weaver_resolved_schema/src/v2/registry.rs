//! A semantic convention registry.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::v2::{
    attribute::{Attribute, AttributeRef},
    entity::Entity,
    event::Event,
    metric::Metric,
    span::Span,
};

/// A semantic convention registry.
///
/// The semantic convention is composed of definitions of
/// attributes, metrics, logs, etc. that will be sent over the wire (e.g. OTLP).
///
/// Note: The registry does not include signal refinements.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// Catalog of attributes used in the schema.
    pub attributes: Vec<Attribute>,

    /// The semantic convention registry url.
    ///
    /// This is the base URL, under which this registry can be found.
    pub registry_url: String,

    /// A  list of span signal definitions.
    pub spans: Vec<Span>,

    /// A  list of metric signal definitions.
    pub metrics: Vec<Metric>,

    /// A  list of event signal definitions.
    pub events: Vec<Event>,

    /// A  list of entity signal definitions.
    pub entities: Vec<Entity>,
}

impl Registry {
    /// Returns the attribute from an attribute ref if it exists.
    #[must_use]
    pub fn attribute(&self, attribute_ref: &AttributeRef) -> Option<&Attribute> {
        self.attributes.get(attribute_ref.0 as usize)
    }
    /// Returns the attribute name from an attribute ref if it exists
    /// in the catalog or None if it does not exist.
    #[must_use]
    pub fn attribute_key(&self, attribute_ref: &AttributeRef) -> Option<&str> {
        self.attributes
            .get(attribute_ref.0 as usize)
            .map(|attr| attr.key.as_ref())
    }
}
