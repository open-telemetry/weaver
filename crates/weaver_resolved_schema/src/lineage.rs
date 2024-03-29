// SPDX-License-Identifier: Apache-2.0

//! Data structures used to keep track of the lineage of a semantic convention.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use weaver_semconv::attribute::{AttributeSpec, Examples, RequirementLevel};
use weaver_semconv::stability::Stability;

/// Resolution mode.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResolutionMode {
    /// Represents the resolution of a reference.
    Reference,
    /// Represents the resolution of an `extends` clause.
    Extends,
}

/// Field id.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash, Eq, Ord, PartialOrd)]
pub enum FieldId {
    /// The group id.
    GroupId,
    /// The group brief.
    GroupBrief,
    /// The group note.
    GroupNote,
    /// The group prefix.
    GroupPrefix,
    /// The group extends.
    GroupExtends,
    /// The group stability.
    GroupStability,
    /// The group deprecated.
    GroupDeprecated,
    /// The group constraints.
    GroupConstraints,
    /// The group attributes.
    GroupAttributes,

    /// The span kind.
    SpanKind,
    /// The span event.
    SpanEvent,

    /// The event name.
    EventName,

    /// The metric name.
    MetricName,
    /// The metric instrument type.
    MetricInstrument,
    /// The metric unit.
    MetricUnit,

    /// The attribute brief.
    AttributeBrief,
    /// The attribute examples.
    AttributeExamples,
    /// The attribute tag.
    AttributeTag,
    /// The attribute requirement level.
    AttributeRequirementLevel,
    /// The attribute sampling relevant.
    AttributeSamplingRelevant,
    /// The attribute note.
    AttributeNote,
    /// The attribute stability.
    AttributeStability,
    /// The attribute deprecated.
    AttributeDeprecated,
    /// The attribute tags.
    AttributeTags,
    /// The attribute value.
    AttributeValue,
}

/// Field lineage.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FieldLineage {
    /// The resolution mode used to resolve the field.
    pub resolution_mode: ResolutionMode,
    /// The id of the group where the field is defined.
    pub group_id: String,
}

/// The lineage information for each field of an attribute.
///
/// Note: By convention, a field not defined in the attribute declaration is
/// None.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct AttributeLineage {
    /// The attribute id.
    pub id: String,
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

// ToDo rename this struct FieldLineage
/// The lineage information of a field.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum FieldProvenance {
    /// The field is locally defined.
    Local,
    /// The field is redefined locally at the reference level.
    Override {
        /// The entity id where the override occurred.
        r#in: String,
    },
    /// The field is inherited from a parent entity pointed by the
    /// `extends` clause.
    /// In semantic conventions, this is used to inherit fields from a parent
    /// group.
    /// In telemetry schemas, this is used to inherit fields from a parent
    /// signal.
    Inherited {
        /// The id pointed by the `extends` clause. This id could be a group id
        /// or a signal id.
        from: String,
    },
}

/// Group lineage.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[must_use]
pub struct GroupLineage {
    /// The path or URL of the source file where the group is defined.
    source_file: String,

    /// The lineage per group field.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    fields: BTreeMap<FieldId, FieldLineage>,

    /// The lineage per attribute.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    attributes: Vec<AttributeLineage>,
}

impl AttributeLineage {
    /// Creates a new attribute lineage.
    #[must_use]
    pub fn new(attr_id: &str, source_group: &str) -> Self {
        Self {
            id: attr_id.to_owned(),
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
            id: attr_spec.id().clone(),
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
        local_value: &Option<String>,
        parent_value: &Option<String>,
    ) -> Option<String> {
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
}

impl GroupLineage {
    /// Creates a new group lineage.
    pub fn new(provenance: String) -> Self {
        Self {
            source_file: provenance,
            fields: BTreeMap::new(),
            attributes: Vec::new(),
        }
    }

    /// Adds a group field lineage.
    pub fn add_group_field_lineage(&mut self, field_id: FieldId, field_lineage: FieldLineage) {
        let prev = self.fields.insert(field_id.clone(), field_lineage.clone());
        if prev.is_some() {
            panic!("Group field `{field_id:?}` lineage already exists (prev: {prev:?}, new: {field_lineage:?}). This is a bug.");
        }
    }

    /// Adds an attribute lineage.
    pub fn add_attribute_lineage(&mut self, attribute_lineage: AttributeLineage) {
        self.attributes.push(attribute_lineage);
    }

    /// Returns the provenance of the group.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.source_file
    }

    /// Returns the lineage of the specified field.
    #[must_use]
    pub fn field_lineage(&self, field_id: &FieldId) -> Option<&FieldLineage> {
        self.fields.get(field_id)
    }
}
