//! Span related definitions structs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    attribute::RequirementLevel,
    group::SpanKindSpec,
    v2::{signal_id::SignalId, span::SpanName, CommonFields},
};

use crate::v2::attribute::AttributeRef;

/// The definition of a Span signal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
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
    /// This list is an "any of" list, where a span may be associated with one or more entities, but should
    /// be associated with at least one in this list.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<String>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// A special type of reference to attributes that remembers span-specicific information.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
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
