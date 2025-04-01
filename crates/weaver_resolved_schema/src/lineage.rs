// SPDX-License-Identifier: Apache-2.0

//! Data structures used to keep track of the lineage of a semantic convention.

use schemars::JsonSchema;
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use weaver_semconv::attribute::{AttributeSpec, Examples, RequirementLevel};
use weaver_semconv::deprecated::Deprecated;
use weaver_semconv::provenance::Provenance;
use weaver_semconv::stability::Stability;
use weaver_semconv::YamlValue;

/// Attribute lineage (at the field level).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AttributeLineage {
    /// The group id where the attribute is coming from.
    pub source_group: String,

    /// A list of fields that are inherited from the source group.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    #[serde(default)]
    pub inherited_fields: BTreeSet<String>,

    /// A list of fields that are overridden in the local group.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    #[serde(default)]
    pub locally_overridden_fields: BTreeSet<String>,
}

/// Group lineage.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[must_use]
pub struct GroupLineage {
    /// The provenance of the source file where the group is defined.
    provenance: Provenance,

    /// The lineage per attribute.
    ///
    /// Note: Use a BTreeMap to ensure a deterministic order of attributes.
    /// This is important to keep unit tests stable.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    attributes: BTreeMap<String, AttributeLineage>,
}

impl AttributeLineage {
    /// Creates a new attribute lineage.
    #[must_use]
    pub fn new(source_group: &str) -> Self {
        Self {
            source_group: source_group.to_owned(),
            inherited_fields: Default::default(),
            locally_overridden_fields: Default::default(),
        }
    }

    /// Creates a new attribute lineage by inheriting fields from the specified
    /// source group.
    #[must_use]
    pub fn inherit_from(source_group: &str, attr_spec: &AttributeSpec) -> Self {
        let mut attr_lineage = Self {
            source_group: source_group.to_owned(),
            inherited_fields: Default::default(),
            locally_overridden_fields: Default::default(),
        };
        match attr_spec {
            AttributeSpec::Ref {
                brief,
                examples,
                tag,
                requirement_level,
                sampling_relevant,
                note,
                stability,
                deprecated,
                prefix,
                ..
            } => {
                if brief.is_some() {
                    _ = attr_lineage.inherited_fields.insert("brief".to_owned());
                }
                if examples.is_some() {
                    _ = attr_lineage.inherited_fields.insert("examples".to_owned());
                }
                if tag.is_some() {
                    _ = attr_lineage.inherited_fields.insert("tag".to_owned());
                }
                if requirement_level.is_some() {
                    _ = attr_lineage
                        .inherited_fields
                        .insert("requirement_level".to_owned());
                }
                if sampling_relevant.is_some() {
                    _ = attr_lineage
                        .inherited_fields
                        .insert("sampling_relevant".to_owned());
                }
                if note.is_some() {
                    _ = attr_lineage.inherited_fields.insert("note".to_owned());
                }
                if stability.is_some() {
                    _ = attr_lineage.inherited_fields.insert("stability".to_owned());
                }
                if deprecated.is_some() {
                    _ = attr_lineage
                        .inherited_fields
                        .insert("deprecated".to_owned());
                }
                if *prefix {
                    _ = attr_lineage.inherited_fields.insert("prefix".to_owned());
                }
            }
            AttributeSpec::Id {
                brief,
                examples,
                tag,
                sampling_relevant,
                stability,
                deprecated,
                ..
            } => {
                if brief.is_some() {
                    _ = attr_lineage.inherited_fields.insert("brief".to_owned());
                }
                if examples.is_some() {
                    _ = attr_lineage.inherited_fields.insert("examples".to_owned());
                }
                if tag.is_some() {
                    _ = attr_lineage.inherited_fields.insert("tag".to_owned());
                }
                _ = attr_lineage
                    .inherited_fields
                    .insert("requirement_level".to_owned());
                if sampling_relevant.is_some() {
                    _ = attr_lineage
                        .inherited_fields
                        .insert("sampling_relevant".to_owned());
                }
                _ = attr_lineage.inherited_fields.insert("note".to_owned());
                if stability.is_some() {
                    _ = attr_lineage.inherited_fields.insert("stability".to_owned());
                }
                if deprecated.is_some() {
                    _ = attr_lineage
                        .inherited_fields
                        .insert("deprecated".to_owned());
                }
            }
        }
        attr_lineage
    }

    /// Determines if the attribute lineage is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inherited_fields.is_empty() && self.locally_overridden_fields.is_empty()
    }

    /// Determines the value of the brief field by evaluating the presence of a
    /// local value. If a local value is provided, it is used, and the brief
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the brief field's lineage is marked as inherited
    /// from the parent.
    pub fn brief(&mut self, local_value: &Option<String>, parent_value: &str) -> String {
        if let Some(local_value) = local_value {
            _ = self.locally_overridden_fields.insert("brief".to_owned());
            _ = self.inherited_fields.remove("brief");
            local_value.clone()
        } else {
            _ = self.inherited_fields.insert("brief".to_owned());
            parent_value.to_owned()
        }
    }

    /// Determines the value of the brief field by evaluating the presence of a
    /// local value. If a local value is provided, it is used, and the brief
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the brief field's lineage is marked as inherited
    /// from the parent.
    pub fn optional_brief(
        &mut self,
        local_value: &Option<String>,
        parent_value: &Option<String>,
    ) -> Option<String> {
        if local_value.is_some() {
            _ = self.locally_overridden_fields.insert("brief".to_owned());
            _ = self.inherited_fields.remove("brief");
            local_value.clone()
        } else {
            if parent_value.is_some() {
                _ = self.inherited_fields.insert("brief".to_owned());
            }
            parent_value.clone()
        }
    }

    /// Determines the value of the note field by evaluating the presence of a
    /// local value. If a local value is provided, it is used, and the note
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the note field's lineage is marked as inherited
    /// from the parent.
    pub fn note(&mut self, local_value: &Option<String>, parent_value: &str) -> String {
        if let Some(local_value) = local_value {
            _ = self.locally_overridden_fields.insert("note".to_owned());
            _ = self.inherited_fields.remove("note");
            local_value.clone()
        } else {
            _ = self.inherited_fields.insert("note".to_owned());
            parent_value.to_owned()
        }
    }

    /// Determines the value of the note field by evaluating the presence of a
    /// local value. If a local value is provided, it is used, and the note
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the note field's lineage is marked as inherited
    /// from the parent.
    pub fn optional_note(
        &mut self,
        local_value: &Option<String>,
        parent_value: &Option<String>,
    ) -> Option<String> {
        if local_value.is_some() {
            _ = self.locally_overridden_fields.insert("note".to_owned());
            _ = self.inherited_fields.remove("note");
            local_value.clone()
        } else {
            if parent_value.is_some() {
                _ = self.inherited_fields.insert("note".to_owned());
            }
            parent_value.clone()
        }
    }

    /// Determines the value of the value field by evaluating the presence of a
    /// local value. If a local value is provided, it is used, and the value
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the value field's lineage is marked as inherited
    /// from the parent.
    pub fn requirement_level(
        &mut self,
        local_value: &Option<RequirementLevel>,
        parent_value: &RequirementLevel,
    ) -> RequirementLevel {
        if let Some(local_value) = local_value {
            _ = self
                .locally_overridden_fields
                .insert("requirement_level".to_owned());
            _ = self.inherited_fields.remove("requirement_level");
            local_value.clone()
        } else {
            _ = self.inherited_fields.insert("requirement_level".to_owned());
            parent_value.to_owned()
        }
    }

    /// Determines the value of the value field by evaluating the presence of a
    /// local value. If a local value is provided, it is used, and the value
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the value field's lineage is marked as inherited
    /// from the parent.
    pub fn optional_requirement_level(
        &mut self,
        local_value: &Option<RequirementLevel>,
        parent_value: &Option<RequirementLevel>,
    ) -> Option<RequirementLevel> {
        if local_value.is_some() {
            _ = self
                .locally_overridden_fields
                .insert("requirement_level".to_owned());
            _ = self.inherited_fields.remove("requirement_level");
            local_value.clone()
        } else {
            if parent_value.is_some() {
                _ = self.inherited_fields.insert("requirement_level".to_owned());
            }
            parent_value.clone()
        }
    }

    /// Determines the value of the examples field by evaluating the presence of
    /// a local value. If a local value is provided, it is used, and the examples
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the examples field's lineage is marked as inherited
    /// from the parent.
    pub fn examples(
        &mut self,
        local_value: &Option<Examples>,
        parent_value: &Option<Examples>,
    ) -> Option<Examples> {
        if local_value.is_some() {
            _ = self.locally_overridden_fields.insert("examples".to_owned());
            _ = self.inherited_fields.remove("examples");
            local_value.clone()
        } else {
            if parent_value.is_some() {
                _ = self.inherited_fields.insert("examples".to_owned());
            }
            parent_value.clone()
        }
    }

    /// Determines the value of the stability field by evaluating the presence of
    /// a local value. If a local value is provided, it is used, and the stability
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the stability field's lineage is marked as inherited
    /// from the parent.
    pub fn stability(
        &mut self,
        local_value: &Option<Stability>,
        parent_value: &Option<Stability>,
    ) -> Option<Stability> {
        if local_value.is_some() {
            _ = self
                .locally_overridden_fields
                .insert("stability".to_owned());
            _ = self.inherited_fields.remove("stability");
            local_value.clone()
        } else {
            if parent_value.is_some() {
                _ = self.inherited_fields.insert("stability".to_owned());
            }
            parent_value.clone()
        }
    }

    /// Determines the value of the deprecated field by evaluating the presence of
    /// a local value. If a local value is provided, it is used, and the deprecated
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the deprecated field's lineage is marked as inherited
    /// from the parent.
    pub fn deprecated(
        &mut self,
        local_value: &Option<Deprecated>,
        parent_value: &Option<Deprecated>,
    ) -> Option<Deprecated> {
        if local_value.is_some() {
            _ = self
                .locally_overridden_fields
                .insert("deprecated".to_owned());
            _ = self.inherited_fields.remove("deprecated");
            local_value.clone()
        } else {
            if parent_value.is_some() {
                _ = self.inherited_fields.insert("deprecated".to_owned());
            }
            parent_value.clone()
        }
    }

    /// Determines the value of the tag field by evaluating the presence of
    /// a local value. If a local value is provided, it is used, and the tag
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the tag field's lineage is marked as inherited
    /// from the parent.
    /// This method updates the lineage information for the tag field to
    /// reflect the source of its value.
    pub fn tag(
        &mut self,
        local_value: &Option<String>,
        parent_value: &Option<String>,
    ) -> Option<String> {
        if local_value.is_some() {
            _ = self.locally_overridden_fields.insert("tag".to_owned());
            _ = self.inherited_fields.remove("tag");
            local_value.clone()
        } else {
            if parent_value.is_some() {
                _ = self.inherited_fields.insert("tag".to_owned());
            }
            parent_value.clone()
        }
    }

    /// Determines the value of the annotations field by evaluating the presence of
    /// a local value. If a local value is provided, it is used, and the annotations
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the tag field's lineage is marked as inherited
    /// from the parent.
    /// This method updates the lineage information for the annotations field to
    /// reflect the source of its value.
    pub fn annotations(
        &mut self,
        local_value: &Option<BTreeMap<String, YamlValue>>,
        parent_value: &Option<BTreeMap<String, YamlValue>>,
    ) -> Option<BTreeMap<String, YamlValue>> {
        if local_value.is_some() {
            _ = self
                .locally_overridden_fields
                .insert("annotations".to_owned());
            _ = self.inherited_fields.remove("annotations");
            local_value.clone()
        } else {
            if parent_value.is_some() {
                _ = self.inherited_fields.insert("annotations".to_owned());
            }
            parent_value.clone()
        }
    }

    /// Determines the value of the tags field by evaluating the presence of
    /// a local value. If a local value is provided, it is used, and the tags
    /// field's lineage is marked as local. Otherwise, the specified parent
    /// value is used, and the tags field's lineage is marked as inherited
    /// from the parent.
    /// This method updates the lineage information for the tags field to
    /// reflect the source of its value.
    pub fn sampling_relevant(
        &mut self,
        local_value: &Option<bool>,
        parent_value: &Option<bool>,
    ) -> Option<bool> {
        if local_value.is_some() {
            _ = self
                .locally_overridden_fields
                .insert("sampling_relevant".to_owned());
            _ = self.inherited_fields.remove("sampling_relevant");
            *local_value
        } else {
            if parent_value.is_some() {
                _ = self.inherited_fields.insert("sampling_relevant".to_owned());
            }
            *parent_value
        }
    }

    /// This method updates the lineage information for the prefix field to
    /// reflect the source of its value.
    pub fn prefix(&mut self, local_value: &bool, parent_value: &bool) -> bool {
        if *local_value {
            _ = self.locally_overridden_fields.insert("prefix".to_owned());
            _ = self.inherited_fields.remove("prefix");
            *local_value
        } else {
            if *parent_value {
                _ = self.inherited_fields.insert("prefix".to_owned());
            }
            *parent_value
        }
    }
}

impl GroupLineage {
    /// Creates a new group lineage.
    pub fn new(provenance: Provenance) -> Self {
        Self {
            provenance,
            attributes: Default::default(),
        }
    }

    /// Adds an attribute lineage.
    pub fn add_attribute_lineage(&mut self, attr_id: String, attribute_lineage: AttributeLineage) {
        _ = self.attributes.insert(attr_id, attribute_lineage);
    }

    /// Checks if a given attribute is present in the group lineage.
    #[must_use]
    pub fn has_attribute(&self, attr_id: &str) -> bool {
        self.attributes.contains_key(attr_id)
    }

    /// Returns the source file of the group (path or URL).
    #[must_use]
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}
