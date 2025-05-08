// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! Specification of a resolved attribute.

use crate::tags::Tags;
use crate::value::Value;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Display;
use std::ops::Not;
#[cfg(test)]
use weaver_semconv::attribute::PrimitiveOrArrayTypeSpec;
use weaver_semconv::attribute::{
    AttributeRole, AttributeSpec, AttributeType, Examples, RequirementLevel,
};
use weaver_semconv::deprecated::Deprecated;
use weaver_semconv::stability::Stability;
use weaver_semconv::YamlValue;

/// An attribute definition.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Attribute {
    /// Attribute name.
    pub name: String,
    /// Either a string literal denoting the type as a primitive or an
    /// array type, a template type or an enum definition.
    pub r#type: AttributeType,
    /// A brief description of the attribute.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub brief: String,
    /// Sequence of example values for the attribute or single example
    /// value. They are required only for string and string array
    /// attributes. Example values must be of the same type of the
    /// attribute. If only a single example is provided, it can directly
    /// be reported without encapsulating it into a sequence/dictionary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Examples>,
    /// Associates a tag ("sub-group") to the attribute. It carries no
    /// particular semantic meaning but can be used e.g. for filtering
    /// in the markdown generator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as <condition> MUST
    /// specify the conditions under which the attribute is required.
    pub requirement_level: RequirementLevel,
    /// Specifies if the attribute is (especially) relevant for sampling
    /// and thus should be set at span start. It defaults to false.
    /// Note: this field is experimental.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling_relevant: Option<bool>,
    /// A more elaborate description of the attribute.
    /// It defaults to an empty string.
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(default)]
    pub note: String,
    /// Specifies the stability of the attribute.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the attribute is deprecated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// Specifies the prefix of the attribute.
    /// If this parameter is set, the resolved id of the referenced attribute will
    /// have group prefix added to it.
    /// It defaults to false.
    #[serde(default)]
    #[serde(skip_serializing_if = "<&bool>::not")]
    pub prefix: bool,
    /// A set of tags for the attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Tags>,
    /// Annotations for the group.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, YamlValue>>,

    /// The value of the attribute.
    /// Note: This is only used in a telemetry schema specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Whether the attribute is identifying or descriptive.
    #[serde(default)]
    #[serde(skip_serializing_if = "AttributeRole::is_identifying")]
    pub role: AttributeRole,
}

/// An unresolved attribute definition.
#[derive(Debug, Deserialize, Clone)]
pub struct UnresolvedAttribute {
    /// The attribute specification.
    pub spec: AttributeSpec,
}

/// An internal reference to an attribute in the catalog.
#[derive(
    Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, JsonSchema, Hash,
)]
pub struct AttributeRef(pub u32);

impl Display for AttributeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AttributeRef({})", self.0)
    }
}

impl Attribute {
    /// Creates a new string attribute.
    /// Note: This constructor is used for testing purposes.
    #[cfg(test)]
    pub(crate) fn string<S: AsRef<str>>(name: S, brief: S, note: S) -> Self {
        Self {
            name: name.as_ref().to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: brief.as_ref().to_owned(),
            examples: None,
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: note.as_ref().to_owned(),
            stability: None,
            deprecated: None,
            prefix: false,
            tags: None,
            value: None,
            annotations: None,
            role: Default::default(),
        }
    }

    /// Creates a new integer attribute.
    /// Note: This constructor is used for testing purposes.
    #[cfg(test)]
    pub(crate) fn int<S: AsRef<str>>(name: S, brief: S, note: S) -> Self {
        Self {
            name: name.as_ref().to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
            brief: brief.as_ref().to_owned(),
            examples: None,
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: note.as_ref().to_owned(),
            stability: None,
            deprecated: None,
            prefix: false,
            tags: None,
            value: None,
            annotations: None,
            role: Default::default(),
        }
    }

    /// Creates a new double attribute.
    /// Note: This constructor is used for testing purposes.
    #[cfg(test)]
    pub(crate) fn double<S: AsRef<str>>(name: S, brief: S, note: S) -> Self {
        Self {
            name: name.as_ref().to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Double),
            brief: brief.as_ref().to_owned(),
            examples: None,
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: note.as_ref().to_owned(),
            stability: None,
            deprecated: None,
            prefix: false,
            tags: None,
            value: None,
            annotations: None,
            role: Default::default(),
        }
    }

    /// Creates a new boolean attribute.
    /// Note: This constructor is used for testing purposes.
    #[cfg(test)]
    pub(crate) fn boolean(
        name: impl Into<String>,
        brief: impl Into<String>,
        note: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into().to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Boolean),
            brief: brief.into().to_owned(),
            examples: None,
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: note.into().to_owned(),
            stability: None,
            deprecated: None,
            prefix: false,
            tags: None,
            value: None,
            annotations: None,
            role: Default::default(),
        }
    }

    /// Sets the deprecated field of the attribute.
    /// Note: This method is used for testing purposes.
    #[cfg(test)]
    pub(crate) fn deprecated(mut self, deprecated: Deprecated) -> Self {
        self.deprecated = Some(deprecated);
        self
    }

    /// Sets the note field of the attribute.
    /// Note: This method is used for testing purposes.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn note<S: AsRef<str>>(mut self, note: S) -> Self {
        self.note = note.as_ref().to_owned();
        self
    }

    /// Sets the brief field of the attribute.
    /// Note: This method is used for testing purposes.
    #[cfg(test)]
    pub(crate) fn brief<S: AsRef<str>>(mut self, brief: S) -> Self {
        self.brief = brief.as_ref().to_owned();
        self
    }
}
