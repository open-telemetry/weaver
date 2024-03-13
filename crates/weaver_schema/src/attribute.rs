// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! Definition of an attribute in the context of a telemetry schema.

use serde::{Deserialize, Serialize};

use weaver_semconv::attribute::{AttributeType, Examples, RequirementLevel, ValueSpec};
use weaver_semconv::stability::Stability;

use crate::tags::Tags;
use crate::Error;

/// An attribute specification.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
#[serde(rename_all = "snake_case")]
pub enum Attribute {
    /// Reference to another attribute.
    ///
    /// ref MUST have an id of an existing attribute.
    /// ref is useful for specifying that an existing attribute of another
    /// semantic convention is part of the current semantic convention and
    /// inherit its brief, note, and example values. However, if these fields
    /// are present in the current attribute definition, they override the
    /// inherited values.
    Ref {
        /// Reference an existing attribute.
        r#ref: String,
        /// A brief description of the attribute.
        #[serde(skip_serializing_if = "Option::is_none")]
        brief: Option<String>,
        /// Sequence of example values for the attribute or single example
        /// value. They are required only for string and string array
        /// attributes. Example values must be of the same type of the
        /// attribute. If only a single example is provided, it can directly
        /// be reported without encapsulating it into a sequence/dictionary.
        #[serde(skip_serializing_if = "Option::is_none")]
        examples: Option<Examples>,
        /// Associates a tag ("sub-group") to the attribute. It carries no
        /// particular semantic meaning but can be used e.g. for filtering
        /// in the markdown generator.
        #[serde(skip_serializing_if = "Option::is_none")]
        tag: Option<String>,
        /// Specifies if the attribute is mandatory. Can be "required",
        /// "conditionally_required", "recommended" or "opt_in". When omitted,
        /// the attribute is "recommended". When set to
        /// "conditionally_required", the string provided as <condition> MUST
        /// specify the conditions under which the attribute is required.
        #[serde(skip_serializing_if = "Option::is_none")]
        requirement_level: Option<RequirementLevel>,
        /// Specifies if the attribute is (especially) relevant for sampling
        /// and thus should be set at span start. It defaults to false.
        /// Note: this field is experimental.
        #[serde(skip_serializing_if = "Option::is_none")]
        sampling_relevant: Option<bool>,
        /// A more elaborate description of the attribute.
        /// It defaults to an empty string.
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        /// Specifies the stability of the attribute.
        /// Note that, if stability is missing but deprecated is present, it will
        /// automatically set the stability to deprecated. If deprecated is
        /// present and stability differs from deprecated, this will result in an
        /// error.
        #[serde(skip_serializing_if = "Option::is_none")]
        stability: Option<Stability>,
        /// Specifies if the attribute is deprecated. The string
        /// provided as <description> MUST specify why it's deprecated and/or what
        /// to use instead. See also stability.
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<String>,
        /// A set of tags for the attribute.
        #[serde(skip_serializing_if = "Option::is_none")]
        tags: Option<Tags>,

        /// The value of the attribute.
        /// Note: This is only used in a telemetry schema specification.
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<ValueSpec>,
    },
    /// Reference to an attribute group.
    ///
    /// `attribute_group_ref` MUST have an id of an existing attribute.
    AttributeGroupRef {
        /// Reference an existing attribute group.
        attribute_group_ref: String,
        /// A set of tags for the attribute.
        #[serde(skip_serializing_if = "Option::is_none")]
        tags: Option<Tags>,
    },
    /// Reference to a span group, i.e. a group of attributes used in the context of
    /// a span.
    ///
    /// `span_ref` MUST have an id of an existing span.
    SpanRef {
        /// Reference an existing span.
        span_ref: String,
        /// A set of tags for the attribute.
        #[serde(skip_serializing_if = "Option::is_none")]
        tags: Option<Tags>,
    },
    /// Reference to a resource group, i.e. a group of attributes used in the context of
    /// a resource.
    ///
    /// `resource_ref` MUST have an id of an existing resource.
    ResourceRef {
        /// Reference an existing resource.
        resource_ref: String,
        /// A set of tags for the attribute.
        #[serde(skip_serializing_if = "Option::is_none")]
        tags: Option<Tags>,
    },
    /// Reference to an event group, i.e. a group of attributes used in the context of
    /// an event.
    ///
    /// `event_ref` MUST have an id of an existing event.
    EventRef {
        /// Reference an existing event.
        event_ref: String,
        /// A set of tags for the attribute.
        #[serde(skip_serializing_if = "Option::is_none")]
        tags: Option<Tags>,
    },
    /// Attribute definition.
    Id {
        /// String that uniquely identifies the attribute.
        id: String,
        /// Either a string literal denoting the type as a primitive or an
        /// array type, a template type or an enum definition.
        r#type: AttributeType,
        /// A brief description of the attribute.
        brief: String,
        /// Sequence of example values for the attribute or single example
        /// value. They are required only for string and string array
        /// attributes. Example values must be of the same type of the
        /// attribute. If only a single example is provided, it can directly
        /// be reported without encapsulating it into a sequence/dictionary.
        // #[serde(skip_serializing_if = "Option::is_none")]
        examples: Option<Examples>,
        /// Associates a tag ("sub-group") to the attribute. It carries no
        /// particular semantic meaning but can be used e.g. for filtering
        /// in the markdown generator.
        #[serde(skip_serializing_if = "Option::is_none")]
        tag: Option<String>,
        /// Specifies if the attribute is mandatory. Can be "required",
        /// "conditionally_required", "recommended" or "opt_in". When omitted,
        /// the attribute is "recommended". When set to
        /// "conditionally_required", the string provided as <condition> MUST
        /// specify the conditions under which the attribute is required.
        #[serde(default)]
        requirement_level: RequirementLevel,
        /// Specifies if the attribute is (especially) relevant for sampling
        /// and thus should be set at span start. It defaults to false.
        /// Note: this field is experimental.
        #[serde(skip_serializing_if = "Option::is_none")]
        sampling_relevant: Option<bool>,
        /// A more elaborate description of the attribute.
        /// It defaults to an empty string.
        #[serde(default)]
        note: String,
        /// Specifies the stability of the attribute.
        /// Note that, if stability is missing but deprecated is present, it will
        /// automatically set the stability to deprecated. If deprecated is
        /// present and stability differs from deprecated, this will result in an
        /// error.
        #[serde(skip_serializing_if = "Option::is_none")]
        stability: Option<Stability>,
        /// Specifies if the attribute is deprecated. The string
        /// provided as <description> MUST specify why it's deprecated and/or what
        /// to use instead. See also stability.
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<String>,
        /// A set of tags for the attribute.
        #[serde(skip_serializing_if = "Option::is_none")]
        tags: Option<Tags>,

        /// The value of the attribute.
        /// Note: This is only used in a telemetry schema specification.
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<ValueSpec>,
    },
}

impl From<&weaver_semconv::attribute::AttributeSpec> for Attribute {
    /// Convert a semantic convention attribute to a schema attribute.
    fn from(attr: &weaver_semconv::attribute::AttributeSpec) -> Self {
        match attr.clone() {
            weaver_semconv::attribute::AttributeSpec::Ref {
                r#ref,
                brief,
                examples,
                tag,
                requirement_level,
                sampling_relevant,
                note,
                stability,
                deprecated,
            } => Attribute::Ref {
                r#ref,
                brief,
                examples,
                tag,
                requirement_level,
                sampling_relevant,
                note,
                stability,
                deprecated,
                tags: None,
                value: None,
            },
            weaver_semconv::attribute::AttributeSpec::Id {
                id,
                r#type,
                brief,
                examples,
                tag,
                requirement_level,
                sampling_relevant,
                note,
                stability,
                deprecated,
            } => Attribute::Id {
                id,
                r#type,
                brief: brief.unwrap_or_default(),
                examples,
                tag,
                requirement_level,
                sampling_relevant,
                note,
                stability,
                deprecated,
                tags: None,
                value: None,
            },
        }
    }
}

/// Convert a slice of semantic convention attributes to a vector of schema attributes.
pub fn to_schema_attributes(attrs: &[weaver_semconv::attribute::AttributeSpec]) -> Vec<Attribute> {
    attrs.iter().map(|attr| attr.into()).collect()
}

impl Attribute {
    /// Returns the id or the reference of the attribute.
    pub fn id(&self) -> String {
        match self {
            Attribute::Ref { r#ref, .. } => r#ref.clone(),
            Attribute::AttributeGroupRef {
                attribute_group_ref,
                ..
            } => attribute_group_ref.clone(),
            Attribute::SpanRef { span_ref, .. } => span_ref.clone(),
            Attribute::ResourceRef { resource_ref, .. } => resource_ref.clone(),
            Attribute::EventRef { event_ref, .. } => event_ref.clone(),
            Attribute::Id { id, .. } => id.clone(),
        }
    }

    /// Sets the tags of the attribute.
    pub fn set_tags(&mut self, tags: &Option<Tags>) {
        match self {
            Attribute::Ref { tags: tags_ref, .. } => {
                tags_ref.clone_from(tags);
            }
            Attribute::Id { tags: tags_id, .. } => {
                tags_id.clone_from(tags);
            }
            Attribute::AttributeGroupRef {
                tags: tags_group, ..
            } => {
                tags_group.clone_from(tags);
            }
            Attribute::ResourceRef {
                tags: tags_resource,
                ..
            } => {
                tags_resource.clone_from(tags);
            }
            Attribute::SpanRef {
                tags: span_tags, ..
            } => {
                span_tags.clone_from(tags);
            }
            Attribute::EventRef {
                tags: event_tags, ..
            } => {
                event_tags.clone_from(tags);
            }
        }
    }

    /// Returns a resolved attribute. The current attribute is expected to be a reference to another
    /// attribute. The semantic convention attribute provided as argument is used to resolve the
    /// reference. The semantic attribute must be an `Attribute::Id` otherwise an error is returned.
    pub fn resolve_from(
        &self,
        sem_conv_attr: Option<&weaver_semconv::attribute::AttributeSpec>,
    ) -> Result<Attribute, Error> {
        match self {
            Attribute::Ref {
                r#ref,
                brief: brief_from_ref,
                examples: examples_from_ref,
                tag: tag_from_ref,
                requirement_level: requirement_level_from_ref,
                sampling_relevant: sampling_from_ref,
                note: note_from_ref,
                stability: stability_from_ref,
                deprecated: deprecated_from_ref,
                tags: tags_from_ref,
                value: value_from_ref,
            } => {
                if let Some(weaver_semconv::attribute::AttributeSpec::Id {
                    id,
                    r#type,
                    brief,
                    examples,
                    tag,
                    requirement_level,
                    sampling_relevant,
                    note,
                    stability,
                    deprecated,
                }) = sem_conv_attr
                {
                    let id = id.clone();
                    let r#type = r#type.clone();
                    let mut brief = brief.clone().unwrap_or_default();
                    let mut examples = examples.clone();
                    let mut requirement_level = requirement_level.clone();
                    let mut tag = tag.clone();
                    let mut sampling_relevant = *sampling_relevant;
                    let mut note = note.clone();
                    let mut stability = stability.clone();
                    let mut deprecated = deprecated.clone();

                    // Override process.
                    // Use the field values from the reference when defined in the reference.
                    if let Some(brief_from_ref) = brief_from_ref {
                        brief.clone_from(brief_from_ref);
                    }
                    if let Some(requirement_level_from_ref) = requirement_level_from_ref {
                        requirement_level = requirement_level_from_ref.clone();
                    }
                    if let Some(examples_from_ref) = examples_from_ref {
                        examples = Some(examples_from_ref.clone());
                    }
                    if let Some(tag_from_ref) = tag_from_ref {
                        tag = Some(tag_from_ref.clone());
                    }
                    if let Some(sampling_from_ref) = sampling_from_ref {
                        sampling_relevant = Some(*sampling_from_ref);
                    }
                    if let Some(note_from_ref) = note_from_ref {
                        note.clone_from(note_from_ref);
                    }
                    if let Some(stability_from_ref) = stability_from_ref {
                        stability = Some(stability_from_ref.clone());
                    }
                    if let Some(deprecated_from_ref) = deprecated_from_ref {
                        deprecated = Some(deprecated_from_ref.clone());
                    }

                    Ok(Attribute::Id {
                        id,
                        r#type,
                        brief,
                        examples,
                        tag,
                        requirement_level,
                        sampling_relevant,
                        note,
                        stability,
                        deprecated,
                        tags: tags_from_ref.clone(),
                        value: value_from_ref.clone(),
                    })
                } else {
                    Err(Error::InvalidAttribute {
                        id: r#ref.clone(),
                        error: "Cannot resolve an attribute from a semantic convention attribute reference.".into(),
                    })
                }
            }
            Attribute::Id { id, .. } => Err(Error::InvalidAttribute {
                id: id.clone(),
                error: "Cannot resolve an attribute from a non-reference attribute.".into(),
            }),
            Attribute::AttributeGroupRef {
                attribute_group_ref,
                ..
            } => Err(Error::InvalidAttribute {
                id: attribute_group_ref.clone(),
                error: "Cannot resolve an attribute from an attribute group reference.".into(),
            }),
            Attribute::SpanRef { span_ref, .. } => Err(Error::InvalidAttribute {
                id: span_ref.clone(),
                error: "Cannot resolve an attribute from a span reference.".into(),
            }),
            Attribute::ResourceRef { resource_ref, .. } => Err(Error::InvalidAttribute {
                id: resource_ref.clone(),
                error: "Cannot resolve an attribute from a resource reference.".into(),
            }),
            Attribute::EventRef { event_ref, .. } => Err(Error::InvalidAttribute {
                id: event_ref.clone(),
                error: "Cannot resolve an attribute from an event reference.".into(),
            }),
        }
    }
}
