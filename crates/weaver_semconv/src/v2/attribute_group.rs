// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define attribute groups going forward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    group::{GroupSpec, GroupType},
    v2::{
        attribute::{split_attributes_and_groups, AttributeOrGroupRef},
        signal_id::SignalId,
        CommonFields,
    },
};

/// Internal attribute group implementation
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InternalAttributeGroup {
    /// The name of the attribute group, must be unique.
    pub id: SignalId,

    /// List of attributes and group references that belong to this group
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeOrGroupRef>,
}

/// Public attribute group implementation
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicAttributeGroup {
    /// The name of the attribute group, must be unique.
    pub id: SignalId,

    /// List of attributes and group references that belong to this group
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeOrGroupRef>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}

/// Attribute group definition.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "visibility")]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum AttributeGroup {
    /// An internal attribute group
    Internal(InternalAttributeGroup),
    /// A public attribute group
    Public(PublicAttributeGroup),
}

// Note: We automatically create the Schemars code and provide `allow(unused_qualifications)` to work around schemars limitations.
// You can use `cargo expand -p weaver_semconv` to find this code and generate it in the future.
const _: () = {
    #[automatically_derived]
    #[allow(unused_braces)]
    impl JsonSchema for AttributeGroup {
        fn schema_name() -> String {
            "AttributeGroup".to_owned()
        }
        fn schema_id() -> std::borrow::Cow<'static, str> {
            std::borrow::Cow::Borrowed("weaver_semconv::v2::attribute_group::AttributeGroup")
        }
        fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
            schemars::_private::metadata::add_description(
                schemars::schema::Schema::Object(schemars::schema::SchemaObject {
                    subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
                        one_of: Some(<[_]>::into_vec(Box::new([
                            schemars::_private::metadata::add_description(
                                schemars::_private::new_internally_tagged_enum(
                                    "visibility",
                                    "internal",
                                    true,
                                ),
                                "An internal attribute group",
                            )
                            .flatten(
                                <InternalAttributeGroup as JsonSchema>::json_schema(generator),
                            ),
                            schemars::_private::metadata::add_description(
                                schemars::_private::new_internally_tagged_enum(
                                    "visibility",
                                    "public",
                                    true,
                                ),
                                "A public attribute group",
                            )
                            .flatten(<PublicAttributeGroup as JsonSchema>::json_schema(generator)),
                        ]))),
                        ..Default::default()
                    })),
                    ..Default::default()
                }),
                "Attribute group definition.",
            )
        }
    }
};

impl AttributeGroup {
    /// Converts a v2 attribute group into a v1 GroupSpec.
    #[must_use]
    pub fn into_v1_group(self) -> GroupSpec {
        match self {
            AttributeGroup::Internal(internal) => {
                let (attribute_refs, include_groups) =
                    split_attributes_and_groups(internal.attributes);

                GroupSpec {
                    id: format!("{}", &internal.id),
                    r#type: GroupType::AttributeGroup,
                    brief: format!("{}", &internal.id),
                    note: "".to_owned(),
                    prefix: Default::default(),
                    extends: None,
                    include_groups,
                    stability: None,
                    deprecated: None,
                    attributes: attribute_refs,
                    span_kind: None,
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                    entity_associations: vec![],
                    visibility: Some(AttributeGroupVisibilitySpec::Internal),
                }
            }
            AttributeGroup::Public(public) => {
                let (attributes, include_groups) = split_attributes_and_groups(public.attributes);

                GroupSpec {
                    id: format!("{}", public.id),
                    r#type: GroupType::AttributeGroup,
                    brief: public.common.brief,
                    note: public.common.note,
                    prefix: Default::default(),
                    extends: None,
                    include_groups,
                    stability: Some(public.common.stability),
                    deprecated: public.common.deprecated,
                    attributes,
                    span_kind: None,
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    display_name: None,
                    body: None,
                    annotations: if public.common.annotations.is_empty() {
                        None
                    } else {
                        Some(public.common.annotations)
                    },
                    entity_associations: vec![],
                    visibility: Some(AttributeGroupVisibilitySpec::Public),
                }
            }
        }
    }
}

/// The group's visibility.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum AttributeGroupVisibilitySpec {
    /// An internal group.
    #[default]
    Internal,
    /// A public group.
    Public,
}

impl std::fmt::Display for AttributeGroupVisibilitySpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttributeGroupVisibilitySpec::Internal => write!(f, "internal"),
            AttributeGroupVisibilitySpec::Public => write!(f, "public"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_translate(v2: &str, v1: &str) {
        let attr_group =
            serde_yaml::from_str::<AttributeGroup>(v2).expect("Failed to parse YAML string");
        let mut expected =
            serde_yaml::from_str::<GroupSpec>(v1).expect("Failed to parse expected YAML");
        // visibility is not serializeable on v1, so let's set it explicitly
        expected.visibility = Some(AttributeGroupVisibilitySpec::Public);
        assert_eq!(expected, attr_group.into_v1_group());
    }

    #[test]
    fn test_value_spec_display() {
        parse_and_translate(
            // V2 - Group
            r#"id: my_attr_group
brief: Test group
stability: development
visibility: public"#,
            // V1 - Group
            r#"id: my_attr_group
type: attribute_group
brief: Test group
stability: development"#,
        );
    }
}
