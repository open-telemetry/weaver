// SPDX-License-Identifier: Apache-2.0

//! Semantic convention specification (Version 1).

use crate::provenance::Provenance;
use crate::v1::group::{GroupSpec, GroupWildcard};
use crate::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_common::result::WResult;

/// A semantic convention file as defined in v1.
/// A semconv file is a collection of semantic convention groups (i.e. [`GroupSpec`]).
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SemConvSpecV1 {
    /// A collection of semantic convention groups or [`GroupSpec`].
    #[serde(default)]
    pub(crate) groups: Vec<GroupSpec>,

    /// A list of imports referencing groups defined in a dependent registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) imports: Option<Imports>,
}

/// Imports are used to reference groups defined in a dependent registry.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Imports {
    /// A list of metric group metric_name wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Vec<GroupWildcard>>,

    /// A list of event group name wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<GroupWildcard>>,

    /// A list of entity group name wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<Vec<GroupWildcard>>,

    /// A list of span group name wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<GroupWildcard>>,

    /// A list of attribute_group group id wildcards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribute_groups: Option<Vec<GroupWildcard>>,
}

/// A wrapper for a [`SemConvSpecV1`] with its provenance.
#[derive(Debug, Clone)]
pub struct SemConvSpecV1WithProvenance {
    /// The semantic convention spec.
    pub spec: SemConvSpecV1,
    /// The provenance of the semantic convention spec (path or URL).
    pub provenance: Provenance,
}

impl SemConvSpecV1 {
    /// Creates a new `SemConvSpecV1` with the given groups and imports.
    #[must_use]
    pub fn new(groups: Vec<GroupSpec>, imports: Option<Imports>) -> Self {
        Self { groups, imports }
    }

    /// Validates the groups in the semantic convention spec.
    pub fn validate(self, provenance: &str) -> WResult<Self, Error> {
        let mut errors: Vec<Error> = vec![];

        for group in &self.groups {
            match group.validate(provenance) {
                WResult::Ok(_) => {}
                WResult::OkWithNFEs(_, errs) => errors.extend(errs),
                WResult::FatalErr(e) => return WResult::FatalErr(e),
            }
        }

        WResult::with_non_fatal_errors(self, errors)
    }

    /// Returns the list of groups in the semantic convention spec.
    #[must_use]
    pub fn groups(&self) -> &[GroupSpec] {
        &self.groups
    }

    /// Returns the list of imports in the semantic convention spec.
    #[must_use]
    pub fn imports(&self) -> Option<&Imports> {
        self.imports.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stability::Stability;
    use crate::v1::attribute::{
        AttributeSpec, AttributeType, BasicRequirementLevelSpec, Examples,
        PrimitiveOrArrayTypeSpec, RequirementLevel,
    };
    use crate::v1::group::{GroupSpec, GroupType};

    #[test]
    fn test_semconv_spec_v1_validate_ok() {
        let group = GroupSpec {
            id: "group.test".to_owned(),
            r#type: GroupType::AttributeGroup,
            brief: "test group".to_owned(),
            attributes: vec![AttributeSpec::Id {
                id: "test.attr".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: Some("test attr".to_owned()),
                examples: Some(Examples::String("example".to_owned())),
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                annotations: None,
                role: None,
            }],
            ..Default::default()
        };

        let spec = SemConvSpecV1::new(vec![group], None);
        assert_eq!(spec.groups().len(), 1);
        assert!(spec.imports().is_none());

        let result = spec.validate("test_prov");
        assert!(matches!(result, WResult::Ok(_)));
    }

    #[test]
    fn test_semconv_spec_v1_validate_with_nfes() {
        let group = GroupSpec {
            id: "group.invalid".to_owned(),
            r#type: GroupType::AttributeGroup,
            brief: "invalid group".to_owned(),
            attributes: vec![AttributeSpec::Id {
                id: "invalid.attr".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: Some("invalid attr".to_owned()),
                // Int example for a String attribute triggers an invalid example error (NFE)
                examples: Some(Examples::Int(12345)),
                tag: None,
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                sampling_relevant: None,
                note: "".to_owned(),
                stability: Some(Stability::Stable),
                deprecated: None,
                annotations: None,
                role: None,
            }],
            ..Default::default()
        };

        let spec = SemConvSpecV1::new(vec![group], None);
        let result = spec.validate("test_prov");
        assert!(matches!(result, WResult::OkWithNFEs(_, errors) if !errors.is_empty()));
    }
}
