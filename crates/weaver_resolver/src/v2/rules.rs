// SPDX-License-Identifier: Apache-2.0

//! Inheritance, validation, and attribute-reference override rules for V2 schemas.

use std::collections::{BTreeMap, HashMap, HashSet};
use weaver_resolved_schema::v2::attribute::Attribute;
use weaver_semconv::deprecated::Deprecated;
use weaver_semconv::provenance::Provenance;
use weaver_semconv::v2::attribute::{
    AttributeRef, AttributeUnref, BasicRequirementLevelSpec, RequirementLevel,
};
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::span::SpanName;
use weaver_semconv::v2::stability::Stability;
use weaver_semconv::v2::CommonFields;
use weaver_semconv::YamlValue;

use crate::error::{AttributeRole, Error};
pub(crate) use crate::graph::merge::merge_annotations;

/// Strips an optional leading `<kind>.` prefix from a refinement ID.
#[must_use]
pub(crate) fn normalize_refinement_id(kind: &str, id: &str) -> SignalId {
    let prefix = match kind {
        "metric" => "metric.",
        "span" => "span.",
        "event" => "event.",
        "entity" => "entity.",
        _ => "",
    };
    if !prefix.is_empty() {
        if let Some(stripped) = id.strip_prefix(prefix) {
            return SignalId::from(stripped);
        }
    }
    SignalId::from(id)
}

/// Uses `refinement_brief` when non-empty; otherwise inherits `parent_brief`.
#[must_use]
pub(crate) fn inherit_brief(refinement_brief: Option<&str>, parent_brief: &str) -> String {
    match refinement_brief {
        Some(b) if !b.is_empty() => b.to_owned(),
        _ => parent_brief.to_owned(),
    }
}

/// Uses `refinement_note` when set (including `Some("")` to clear); otherwise inherits `parent_note`.
#[must_use]
pub(crate) fn inherit_note(
    refinement_note: &Option<String>,
    parent_note: &Option<String>,
) -> Option<String> {
    refinement_note.clone().or_else(|| parent_note.clone())
}

/// Returns `refinement_stability` if set; otherwise inherits `parent_stability`.
#[must_use]
pub(crate) fn inherit_stability(
    refinement_stability: &Option<Stability>,
    parent_stability: &Stability,
) -> Stability {
    refinement_stability
        .clone()
        .unwrap_or_else(|| parent_stability.clone())
}

/// Returns `refinement_deprecated` if set; otherwise inherits `parent_deprecated`.
#[must_use]
pub(crate) fn inherit_deprecated(
    refinement_deprecated: &Option<Deprecated>,
    parent_deprecated: &Option<Deprecated>,
) -> Option<Deprecated> {
    refinement_deprecated
        .clone()
        .or_else(|| parent_deprecated.clone())
}

/// Returns `refinement_name` if set; otherwise inherits `parent_name`.
#[must_use]
pub(crate) fn inherit_span_name(
    refinement_name: &Option<SpanName>,
    parent_name: &SpanName,
) -> SpanName {
    refinement_name
        .clone()
        .unwrap_or_else(|| parent_name.clone())
}

/// Deep-merges parent and refinement annotations, with refinement keys taking precedence.
#[must_use]
pub(crate) fn inherit_annotations(
    parent_annotations: &BTreeMap<String, YamlValue>,
    refinement_annotations: &BTreeMap<String, YamlValue>,
) -> BTreeMap<String, YamlValue> {
    merge_annotations(parent_annotations.clone(), refinement_annotations)
}

/// Validates attribute `unref` entries against parent and refinement constraints.
#[must_use]
pub(crate) fn validate_attribute_unrefs(
    refinement_id: &str,
    parent_ref: &str,
    unrefs: &[AttributeUnref],
    parent_attr_keys: &HashSet<String>,
    parent_required_keys: &HashSet<String>,
    parent_identifying_keys: &HashSet<String>,
    local_attr_keys: &HashSet<String>,
    included_group_attrs: &HashMap<String, String>,
    provenance: Option<&Provenance>,
) -> Vec<Error> {
    let mut seen = HashSet::new();
    let mut errors = Vec::new();

    for u in unrefs {
        let key = &u.unref;
        let reason = if !seen.insert(key.clone()) {
            "duplicate unref entry".to_owned()
        } else if local_attr_keys.contains(key) {
            "the attribute is also explicitly referenced by this refinement".to_owned()
        } else if let Some(source) = included_group_attrs.get(key) {
            format!("the attribute is also included by ref_group `{source}`")
        } else if parent_identifying_keys.contains(key) {
            format!("the attribute identifies entity `{parent_ref}`; refinements must preserve identity")
        } else if parent_required_keys.contains(key) {
            format!("the attribute is required by `{parent_ref}`")
        } else if !parent_attr_keys.contains(key) {
            format!("the attribute is not inherited from `{parent_ref}`")
        } else {
            continue;
        };

        errors.push(Error::InvalidAttributeUnref {
            refinement_id: refinement_id.to_owned(),
            attribute_key: key.clone(),
            reason,
            provenance: provenance.cloned().map(Box::new),
        });
    }

    errors
}

/// Validates that an entity refinement preserves its parent's identifying attribute set.
#[must_use]
pub(crate) fn validate_entity_identity_refinements(
    refinement_id: &str,
    parent_ref: &str,
    parent_identifying_keys: &HashSet<String>,
    refinement_identity_keys: &[String],
    refinement_description_keys: &[String],
    provenance: Option<&Provenance>,
) -> Vec<Error> {
    if parent_identifying_keys.is_empty() {
        return vec![Error::EntityRefinementWithoutIdentity {
            refinement_id: refinement_id.to_owned(),
            r#ref: parent_ref.to_owned(),
            provenance: provenance.cloned().map(Box::new),
        }];
    }

    let mut errors = Vec::new();

    for key in refinement_identity_keys {
        if !parent_identifying_keys.contains(key) {
            errors.push(Error::EntityRefinementChangedIdentity {
                refinement_id: refinement_id.to_owned(),
                r#ref: parent_ref.to_owned(),
                attribute_id: key.clone(),
                role: AttributeRole::Identifying,
                provenance: provenance.cloned().map(Box::new),
            });
        }
    }

    for key in refinement_description_keys {
        if parent_identifying_keys.contains(key) {
            errors.push(Error::EntityRefinementChangedIdentity {
                refinement_id: refinement_id.to_owned(),
                r#ref: parent_ref.to_owned(),
                attribute_id: key.clone(),
                role: AttributeRole::Descriptive,
                provenance: provenance.cloned().map(Box::new),
            });
        }
    }

    errors
}

/// Applies `aref` overrides (`brief`, `note`, `examples`, `annotations`) onto `def`, keeping definition-owned fields intact.
#[must_use]
pub(crate) fn apply_attribute_ref_overrides(def: &Attribute, aref: &AttributeRef) -> Attribute {
    let brief = aref
        .brief
        .clone()
        .unwrap_or_else(|| def.common.brief.clone());
    let note = aref.note.clone().or_else(|| def.common.note.clone());
    let examples = aref.examples.clone().or_else(|| def.examples.clone());
    let annotations = merge_annotations(def.common.annotations.clone(), &aref.annotations);

    Attribute {
        key: def.key.clone(),
        r#type: def.r#type.clone(),
        examples,
        common: CommonFields {
            brief,
            note,
            stability: def.common.stability.clone(),
            deprecated: def.common.deprecated.clone(),
            annotations,
        },
        provenance: def.provenance.clone(),
    }
}

/// Returns `aref.requirement_level`, defaulting to `Recommended`.
#[must_use]
pub(crate) fn resolve_requirement_level(aref: &AttributeRef) -> RequirementLevel {
    aref.requirement_level
        .clone()
        .unwrap_or(RequirementLevel::Basic(
            BasicRequirementLevelSpec::Recommended,
        ))
}

#[cfg(test)]
#[must_use]
pub(crate) fn resolve_identity_requirement_level() -> RequirementLevel {
    RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
}

/// Merges `override_ref` fields onto `base`, deep-merging `annotations`.
#[must_use]
pub(crate) fn merge_attribute_refs(
    base: &AttributeRef,
    override_ref: &AttributeRef,
) -> AttributeRef {
    let brief = override_ref.brief.clone().or_else(|| base.brief.clone());
    let examples = override_ref
        .examples
        .clone()
        .or_else(|| base.examples.clone());
    let requirement_level = override_ref
        .requirement_level
        .clone()
        .or_else(|| base.requirement_level.clone());
    let note = override_ref.note.clone().or_else(|| base.note.clone());
    let annotations = merge_annotations(base.annotations.clone(), &override_ref.annotations);

    AttributeRef {
        r#ref: base.r#ref.clone(),
        brief,
        examples,
        requirement_level,
        note,
        annotations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use weaver_resolved_schema::v2::provenance::{DependencyRef, Provenance};
    use weaver_semconv::deprecated::Deprecated;
    use weaver_semconv::v2::attribute::{AttributeType, Examples, PrimitiveOrArrayTypeSpec};
    use weaver_semconv::v2::stability::Stability;
    use weaver_semconv::v2::CommonFields;
    use weaver_semconv::YamlValue;

    fn sample_definition() -> Attribute {
        let mut annotations = BTreeMap::new();
        _ = annotations.insert(
            "custom.key".to_owned(),
            YamlValue(serde_json::json!("base_val")),
        );
        Attribute {
            key: "test.key".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: Some(Examples::Strings(vec!["example_base".to_owned()])),
            common: CommonFields {
                brief: "Base brief".to_owned(),
                note: Some("Base note".to_owned()),
                stability: Stability::Stable,
                deprecated: Some(Deprecated::Renamed {
                    renamed_to: "other.key".to_owned(),
                    note: None,
                }),
                annotations,
            },
            provenance: Provenance {
                source: Some(DependencyRef(0)),
                path: "path/to/spec.yaml".to_owned(),
            },
        }
    }

    #[test]
    fn test_attribute_ref_no_overrides_inherits_all() {
        let def = sample_definition();
        let aref = AttributeRef {
            r#ref: "test.key".to_owned(),
            brief: None,
            examples: None,
            requirement_level: None,
            note: None,
            annotations: BTreeMap::new(),
        };

        let resolved = apply_attribute_ref_overrides(&def, &aref);
        assert_eq!(resolved.key, "test.key");
        assert_eq!(resolved.common.brief, "Base brief");
        assert_eq!(resolved.common.note.as_deref(), Some("Base note"));
        assert_eq!(
            resolved.examples,
            Some(Examples::Strings(vec!["example_base".to_owned()]))
        );
        assert_eq!(resolved.common.stability, Stability::Stable);
        assert!(resolved.common.deprecated.is_some());
        assert_eq!(resolved.provenance.source, Some(DependencyRef(0)));
        assert_eq!(
            resolved.common.annotations.get("custom.key").unwrap().0,
            serde_json::json!("base_val")
        );
    }

    #[test]
    fn test_attribute_ref_overrides_brief_note_examples_annotations() {
        let def = sample_definition();
        let mut override_annotations = BTreeMap::new();
        _ = override_annotations.insert(
            "custom.key".to_owned(),
            YamlValue(serde_json::json!("overridden_val")),
        );
        _ = override_annotations.insert(
            "extra.key".to_owned(),
            YamlValue(serde_json::json!("new_val")),
        );

        let aref = AttributeRef {
            r#ref: "test.key".to_owned(),
            brief: Some("Overridden brief".to_owned()),
            examples: Some(Examples::Strings(vec!["example_override".to_owned()])),
            requirement_level: Some(RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn)),
            note: Some("Overridden note".to_owned()),
            annotations: override_annotations,
        };

        let resolved = apply_attribute_ref_overrides(&def, &aref);
        assert_eq!(resolved.common.brief, "Overridden brief");
        assert_eq!(resolved.common.note.as_deref(), Some("Overridden note"));
        assert_eq!(
            resolved.examples,
            Some(Examples::Strings(vec!["example_override".to_owned()]))
        );
        // Stability, deprecated, type, provenance stay fixed from definition
        assert_eq!(resolved.common.stability, Stability::Stable);
        assert!(resolved.common.deprecated.is_some());
        assert_eq!(resolved.r#type, def.r#type);
        assert_eq!(resolved.provenance.source, def.provenance.source);

        // Annotations deep merged: overridden key replaced, new key added
        assert_eq!(
            resolved.common.annotations.get("custom.key").unwrap().0,
            serde_json::json!("overridden_val")
        );
        assert_eq!(
            resolved.common.annotations.get("extra.key").unwrap().0,
            serde_json::json!("new_val")
        );
    }

    #[test]
    fn test_requirement_level_resolution() {
        let aref_default = AttributeRef {
            r#ref: "test.key".to_owned(),
            brief: None,
            examples: None,
            requirement_level: None,
            note: None,
            annotations: BTreeMap::new(),
        };
        assert_eq!(
            resolve_requirement_level(&aref_default),
            RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended)
        );

        let aref_explicit = AttributeRef {
            r#ref: "test.key".to_owned(),
            brief: None,
            examples: None,
            requirement_level: Some(RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn)),
            note: None,
            annotations: BTreeMap::new(),
        };
        assert_eq!(
            resolve_requirement_level(&aref_explicit),
            RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn)
        );

        assert_eq!(
            resolve_identity_requirement_level(),
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
        );
    }

    #[test]
    fn test_merge_attribute_refs() {
        let base = AttributeRef {
            r#ref: "server.address".to_owned(),
            brief: Some("base brief".to_owned()),
            examples: None,
            requirement_level: Some(RequirementLevel::Basic(BasicRequirementLevelSpec::Required)),
            note: Some("base note".to_owned()),
            annotations: BTreeMap::from([("tag".to_owned(), YamlValue(serde_json::json!("v1")))]),
        };
        let override_ref = AttributeRef {
            r#ref: "server.address".to_owned(),
            brief: None, // Keep base
            examples: None,
            requirement_level: None, // Keep base
            note: Some("overridden note".to_owned()),
            annotations: BTreeMap::from([("tag2".to_owned(), YamlValue(serde_json::json!("v2")))]),
        };
        let merged = merge_attribute_refs(&base, &override_ref);
        assert_eq!(merged.brief.as_deref(), Some("base brief"));
        assert_eq!(merged.note.as_deref(), Some("overridden note"));
        assert_eq!(
            merged.requirement_level,
            Some(RequirementLevel::Basic(BasicRequirementLevelSpec::Required))
        );
        assert_eq!(merged.annotations.len(), 2);
    }

    #[test]
    fn test_normalize_refinement_id() {
        assert_eq!(
            &*normalize_refinement_id("metric", "metric.foo.bar"),
            "foo.bar"
        );
        assert_eq!(&*normalize_refinement_id("span", "span.my_span"), "my_span");
        assert_eq!(
            &*normalize_refinement_id("event", "event.user.login"),
            "user.login"
        );
        assert_eq!(
            &*normalize_refinement_id("entity", "entity.host.vm"),
            "host.vm"
        );

        // Non-prefixed ids are preserved
        assert_eq!(&*normalize_refinement_id("metric", "foo.bar"), "foo.bar");
        assert_eq!(&*normalize_refinement_id("entity", "host.vm"), "host.vm");

        // Different kind prefix is NOT stripped
        assert_eq!(&*normalize_refinement_id("metric", "span.foo"), "span.foo");
    }

    #[test]
    fn test_inherit_brief() {
        assert_eq!(inherit_brief(Some("new brief"), "base brief"), "new brief");
        assert_eq!(inherit_brief(None, "base brief"), "base brief");
        assert_eq!(inherit_brief(Some(""), "base brief"), "base brief");
    }

    #[test]
    fn test_inherit_note() {
        assert_eq!(
            inherit_note(&Some("new note".to_owned()), &Some("base note".to_owned())),
            Some("new note".to_owned())
        );
        assert_eq!(
            inherit_note(&None, &Some("base note".to_owned())),
            Some("base note".to_owned())
        );
        // Explicitly empty note overrides parent note (does NOT inherit)
        assert_eq!(
            inherit_note(&Some("".to_owned()), &Some("base note".to_owned())),
            Some("".to_owned())
        );
    }

    #[test]
    fn test_inherit_stability_and_deprecated() {
        assert_eq!(
            inherit_stability(&Some(Stability::Stable), &Stability::Development),
            Stability::Stable
        );
        assert_eq!(
            inherit_stability(&None, &Stability::Development),
            Stability::Development
        );

        let dep = Deprecated::Renamed {
            renamed_to: "new.name".to_owned(),
            note: None,
        };
        assert_eq!(
            inherit_deprecated(&Some(dep.clone()), &None),
            Some(dep.clone())
        );
        assert_eq!(inherit_deprecated(&None, &Some(dep.clone())), Some(dep));
    }

    #[test]
    fn test_inherit_span_name() {
        let base_name = SpanName {
            templates: vec![],
            note: Some("base name".to_owned()),
        };
        let override_name = SpanName {
            templates: vec![],
            note: Some("override name".to_owned()),
        };
        assert_eq!(
            inherit_span_name(&Some(override_name.clone()), &base_name),
            override_name
        );
        assert_eq!(inherit_span_name(&None, &base_name), base_name);
    }

    #[test]
    fn test_inherit_annotations() {
        let mut base_ann = BTreeMap::new();
        _ = base_ann.insert("a".to_owned(), YamlValue(serde_json::json!(1)));
        _ = base_ann.insert("b".to_owned(), YamlValue(serde_json::json!(2)));

        let mut child_ann = BTreeMap::new();
        _ = child_ann.insert("b".to_owned(), YamlValue(serde_json::json!(99)));
        _ = child_ann.insert("c".to_owned(), YamlValue(serde_json::json!(3)));

        let merged = inherit_annotations(&base_ann, &child_ann);
        assert_eq!(merged.get("a").unwrap().0, serde_json::json!(1));
        assert_eq!(merged.get("b").unwrap().0, serde_json::json!(99));
        assert_eq!(merged.get("c").unwrap().0, serde_json::json!(3));
    }

    #[test]
    fn test_validate_attribute_unrefs() {
        let parent_attrs = HashSet::from([
            "inherited.opt".to_owned(),
            "inherited.req".to_owned(),
            "host.id".to_owned(),
        ]);
        let parent_req = HashSet::from(["inherited.req".to_owned()]);
        let parent_identifying = HashSet::from(["host.id".to_owned()]);
        let local_refs = HashSet::from(["local.ref".to_owned()]);
        let included = HashMap::from([("inc.attr".to_owned(), "shared.group".to_owned())]);

        let unrefs = vec![AttributeUnref {
            unref: "inherited.opt".to_owned(),
        }];
        let errs = validate_attribute_unrefs(
            "my.refinement",
            "my.base",
            &unrefs,
            &parent_attrs,
            &parent_req,
            &parent_identifying,
            &local_refs,
            &included,
            None,
        );
        assert!(errs.is_empty());

        let unrefs = vec![
            AttributeUnref {
                unref: "inherited.opt".to_owned(),
            },
            AttributeUnref {
                unref: "inherited.opt".to_owned(),
            },
        ];
        let errs = validate_attribute_unrefs(
            "my.refinement",
            "my.base",
            &unrefs,
            &parent_attrs,
            &parent_req,
            &parent_identifying,
            &local_refs,
            &included,
            None,
        );
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            Error::InvalidAttributeUnref { reason, .. } => {
                assert_eq!(reason, "duplicate unref entry");
            }
            other => panic!("Unexpected error: {other:?}"),
        }

        let unrefs = vec![AttributeUnref {
            unref: "local.ref".to_owned(),
        }];
        let errs = validate_attribute_unrefs(
            "my.refinement",
            "my.base",
            &unrefs,
            &parent_attrs,
            &parent_req,
            &parent_identifying,
            &local_refs,
            &included,
            None,
        );
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            Error::InvalidAttributeUnref { reason, .. } => {
                assert!(reason.contains("also explicitly referenced by this refinement"));
            }
            other => panic!("Unexpected error: {other:?}"),
        }

        let unrefs = vec![AttributeUnref {
            unref: "inc.attr".to_owned(),
        }];
        let errs = validate_attribute_unrefs(
            "my.refinement",
            "my.base",
            &unrefs,
            &parent_attrs,
            &parent_req,
            &parent_identifying,
            &local_refs,
            &included,
            None,
        );
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            Error::InvalidAttributeUnref { reason, .. } => {
                assert!(reason.contains("also included by ref_group `shared.group`"));
            }
            other => panic!("Unexpected error: {other:?}"),
        }

        let unrefs = vec![AttributeUnref {
            unref: "host.id".to_owned(),
        }];
        let errs = validate_attribute_unrefs(
            "my.refinement",
            "my.base",
            &unrefs,
            &parent_attrs,
            &parent_req,
            &parent_identifying,
            &local_refs,
            &included,
            None,
        );
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            Error::InvalidAttributeUnref { reason, .. } => {
                assert!(reason
                    .contains("identifies entity `my.base`; refinements must preserve identity"));
            }
            other => panic!("Unexpected error: {other:?}"),
        }

        let unrefs = vec![AttributeUnref {
            unref: "inherited.req".to_owned(),
        }];
        let errs = validate_attribute_unrefs(
            "my.refinement",
            "my.base",
            &unrefs,
            &parent_attrs,
            &parent_req,
            &parent_identifying,
            &local_refs,
            &included,
            None,
        );
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            Error::InvalidAttributeUnref { reason, .. } => {
                assert!(reason.contains("required by `my.base`"));
            }
            other => panic!("Unexpected error: {other:?}"),
        }

        let unrefs = vec![AttributeUnref {
            unref: "not.inherited".to_owned(),
        }];
        let errs = validate_attribute_unrefs(
            "my.refinement",
            "my.base",
            &unrefs,
            &parent_attrs,
            &parent_req,
            &parent_identifying,
            &local_refs,
            &included,
            None,
        );
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            Error::InvalidAttributeUnref { reason, .. } => {
                assert!(reason.contains("not inherited from `my.base`"));
            }
            other => panic!("Unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_validate_entity_identity_refinements() {
        let parent_identifying = HashSet::from(["host.id".to_owned()]);

        // Valid refinement
        let errs = validate_entity_identity_refinements(
            "refined.host",
            "base.host",
            &parent_identifying,
            &["host.id".to_owned()],
            &["host.name".to_owned()],
            None,
        );
        assert!(errs.is_empty());

        // Base has no identity
        let empty_parent = HashSet::new();
        let errs = validate_entity_identity_refinements(
            "refined.host",
            "base.host",
            &empty_parent,
            &[],
            &["host.name".to_owned()],
            None,
        );
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            &errs[0],
            Error::EntityRefinementWithoutIdentity { .. }
        ));

        // Refinement adds new identity attribute
        let errs = validate_entity_identity_refinements(
            "refined.host",
            "base.host",
            &parent_identifying,
            &["host.id".to_owned(), "new.id".to_owned()],
            &[],
            None,
        );
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            Error::EntityRefinementChangedIdentity {
                attribute_id, role, ..
            } => {
                assert_eq!(attribute_id, "new.id");
                assert_eq!(*role, AttributeRole::Identifying);
            }
            other => panic!("Unexpected: {other:?}"),
        }

        // Refinement demotes identity attribute to description
        let errs = validate_entity_identity_refinements(
            "refined.host",
            "base.host",
            &parent_identifying,
            &[],
            &["host.id".to_owned()],
            None,
        );
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            Error::EntityRefinementChangedIdentity {
                attribute_id, role, ..
            } => {
                assert_eq!(attribute_id, "host.id");
                assert_eq!(*role, AttributeRole::Descriptive);
            }
            other => panic!("Unexpected: {other:?}"),
        }
    }
}
