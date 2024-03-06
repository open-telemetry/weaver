// SPDX-License-Identifier: Apache-2.0

//! Registry used during the evaluation of the templates. References to the
//! catalog are resolved to the actual catalog entries to ease the template
//! evaluation.

use serde::{Deserialize, Serialize};
use weaver_resolved_schema::attribute::Attribute;
use weaver_resolved_schema::catalog::{Catalog, Stability};
use weaver_resolved_schema::lineage::GroupLineage;
use weaver_resolved_schema::registry::{Constraint, Registry, TypedGroup};
use crate::Error;

/// A semantic convention registry used in the context of the template engine.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemplateRegistry {
    /// The semantic convention registry url.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub registry_url: String,
    /// A list of semantic convention groups.
    pub groups: Vec<TemplateGroup>,
}

/// Group specification used in the context of the template engine.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TemplateGroup {
    /// The id that uniquely identifies the semantic convention.
    pub id: String,
    /// The type of the group including the specific fields for each type.
    pub typed_group: TypedGroup,
    /// A brief description of the semantic convention.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub brief: String,
    /// A more elaborate description of the semantic convention.
    /// It defaults to an empty string.
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Prefix for the attributes for this semantic convention.
    /// It defaults to an empty string.
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    /// Reference another semantic convention id. It inherits the prefix,
    /// constraints, and all attributes defined in the specified semantic
    /// convention.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
    /// Specifies the stability of the semantic convention.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the semantic convention is deprecated. The string
    /// provided as <description> MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    /// Additional constraints.
    /// Allow to define additional requirements on the semantic convention.
    /// It defaults to an empty list.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<Constraint>,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    pub attributes: Vec<Attribute>,
    /// The lineage of the group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage: Option<GroupLineage>,
}

impl TemplateRegistry {
    /// Create a new template registry from a resolved registry.
    pub fn try_from_resolved_registry(
        registry: &Registry,
        catalog: &Catalog,
    ) -> Result<Self, Error> {
        let mut errors = Vec::new();

        let groups = registry
            .groups
            .iter()
            .map(|group| {
                let id = group.id.clone();
                let typed_group = group.typed_group.clone();
                let brief = group.brief.clone();
                let note = group.note.clone();
                let prefix = group.prefix.clone();
                let extends = group.extends.clone();
                let stability = group.stability.clone();
                let deprecated = group.deprecated.clone();
                let constraints = group.constraints.clone();
                let attributes = group.attributes.iter().flat_map(|attr_ref| {
                    let attr = catalog.attribute(attr_ref).map(|a| a.clone());
                    if attr.is_none() {
                        errors.push(Error::AttributeNotFound {
                            group_id: id.clone(),
                            attr_ref: *attr_ref,
                        });
                    }
                    attr
                }).collect();
                let lineage = group.lineage.clone();
                TemplateGroup {
                    id,
                    typed_group,
                    brief,
                    note,
                    prefix,
                    extends,
                    stability,
                    deprecated,
                    constraints,
                    attributes,
                    lineage,
                }
            })
            .collect();

        if !errors.is_empty() {
            return Err(Error::CompoundError(errors))
        }

        Ok(Self {
            registry_url: registry.registry_url.clone(),
            groups,
        })
    }
}