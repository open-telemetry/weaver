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
