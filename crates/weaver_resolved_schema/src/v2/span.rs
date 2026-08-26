//! Span related definitions structs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    attribute::RequirementLevel,
    group::SpanKindSpec,
    signal_requirement_level::SignalRequirementLevel,
    v2::{signal_id::SignalId, span::SpanName, CommonFields},
};

use crate::v2::{
    attribute::AttributeRef, entity::EntityAssociation, provenance::Provenance, Signal,
};

/// The definition of a Span signal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct Span {
    /// The type of the Span. This denotes the identity
    /// of the "shape" of this span, and must be unique.
    pub r#type: SignalId,
    /// Specifies the kind of the span.
    pub kind: SpanKindSpec,
    /// The name pattern for the span.
    pub name: SpanName,
    // TODO - Should we split attributes into "sampling_relevant" and "other" groups here?
    /// List of attributes that belong to this span.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<SpanAttributeRef>,

    // TODO - Should Entity Associations be "strong" links?
    /// Which entities this span should be associated with.
    ///
    /// The list is an implicit `one_of` (telemetry must satisfy at least one entry); each entry is an
    /// entity reference (a type plus its provenance) or a nested `one_of`/`all_of` expression.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<EntityAssociation>,

    /// Declares links from this span to other spans.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<SpanLink>,

    /// The requirement level of the span. Defaults to 'recommended' when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<SignalRequirementLevel>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,

    /// The provenance of the Span.
    #[serde(default)]
    #[serde(skip_serializing_if = "Provenance::is_empty")]
    pub provenance: Provenance,
}

/// A resolved link from this span to another span.
///
/// Span links model relations that do not fit the parent/child tree,
/// for example a batch consumer span that links to the producer span
/// of each message it processes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct SpanLink {
    /// The span type this link points to.
    pub r#ref: SignalId,
    /// The requirement level of the link. Uses the attribute requirement
    /// levels because a link, unlike a signal, can be required. The
    /// definition-time default ('recommended') is applied during
    /// resolution.
    pub requirement_level: RequirementLevel,
    /// The brief description of the link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// The more elaborate description of the link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// List of attributes expected on the link itself.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<SpanAttributeRef>,
    /// The provenance of the registry that declared the link.
    #[serde(default)]
    #[serde(skip_serializing_if = "Provenance::is_empty")]
    pub provenance: Provenance,
}

/// A special type of reference to attributes that remembers span-specicific information.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct SpanAttributeRef {
    /// Reference, by index, to the attribute catalog.
    pub base: AttributeRef,
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as `condition` MUST
    /// specify the conditions under which the attribute is required.
    pub requirement_level: RequirementLevel,
    /// Specifies if the attribute is (especially) relevant for sampling
    /// and thus should be set at span start. It defaults to false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling_relevant: Option<bool>,
}

/// A refinement of a span, for use in code-gen or specific library application.
///
/// A refinement represents a "view" of a Span that is highly optimised for a particular implementation.
/// e.g. for HTTP spans, there may be a refinement that provides only the necessary information for dealing with Java's HTTP
/// client library, and drops optional or extraneous information from the underlying http span.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SpanRefinement {
    /// The identity of the refinement
    pub id: SignalId,

    // TODO - This is a lazy way of doing this.  We use `type` to refer
    // to the underlying span definition, but override all fields here.
    // We probably should copy-paste all the "span" attributes here
    // including the `ty`
    /// The definition of the span refinement.
    #[serde(flatten)]
    pub span: Span,
}
impl Signal for Span {
    fn id(&self) -> &str {
        &self.r#type
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
}
