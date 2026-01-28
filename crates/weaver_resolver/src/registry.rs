// SPDX-License-Identifier: Apache-2.0

//! Functions to resolve a semantic convention registry.

use crate::attribute::AttributeCatalog;
use crate::dependency::{ImportableDependency, ResolvedDependency};
use crate::Error;
use crate::Error::{DuplicateGroupId, DuplicateGroupName, DuplicateMetricName};
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Display;
use std::hash::Hash;
use weaver_common::result::WResult;
use weaver_resolved_schema::attribute::{AttributeRef, UnresolvedAttribute};
use weaver_resolved_schema::lineage::{AttributeLineage, GroupLineage};
use weaver_resolved_schema::registry::{Group, Registry};
use weaver_semconv::attribute::AttributeSpec;
use weaver_semconv::group::{GroupSpecWithProvenance, GroupType, ImportsWithProvenance};
use weaver_semconv::provenance::Provenance;
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::semconv::{SemConvSpecV1WithProvenance, SemConvSpecWithProvenance};
use weaver_semconv::v2::attribute_group::AttributeGroupVisibilitySpec;

/// A registry containing unresolved groups.
#[derive(Debug, Deserialize)]
pub struct UnresolvedRegistry {
    /// The semantic convention registry containing resolved groups.
    pub registry: Registry,

    /// List of unresolved groups that belong to the registry.
    /// The resolution process will progressively move the unresolved groups
    /// into the registry field once they are resolved.
    pub groups: Vec<UnresolvedGroup>,

    /// List of unresolved imports that belong to the semantic convention
    pub imports: Vec<ImportsWithProvenance>,

    /// List of dependencies we may use when resolving this registry.
    pub(crate) dependencies: Vec<ResolvedDependency>,
}

/// A group containing unresolved attributes.
#[derive(Debug, Deserialize)]
pub struct UnresolvedGroup {
    /// The group specification containing resolved attributes and signals.
    pub group: Group,

    /// List of unresolved attributes that belong to the semantic convention
    /// group.
    /// The resolution process will progressively move the unresolved attributes,
    /// and other signals, into the group field once they are resolved.
    pub attributes: Vec<UnresolvedAttribute>,

    /// List of groups to include in the semantic convention group.
    pub include_groups: Vec<String>,

    /// Visibility of the group.
    pub visibility: Option<AttributeGroupVisibilitySpec>,

    /// The provenance of the group (URL or path).
    pub provenance: Provenance,
}

/// Resolves the semantic convention registry passed as argument and returns
/// the resolved registry or an error if the resolution process failed.
///
/// The resolution process consists of the following steps:
/// - Resolve all attribute references and apply the overrides when needed.
/// - Resolve all the `extends` references.
///
/// # Arguments
///
/// * `attr_catalog` - The attribute catalog to use to resolve the attribute references.
/// * `repo` - The manifest of the registry
/// * `specs` - The raw specifications of the repository
/// * `imports` - Definitions to import from dependencies.
/// * `dependencies` - The resolved schemas of the dependencies of this repository.
///
/// # Returns
///
/// This function returns the resolved registry or an error if the resolution process
/// failed.
pub(crate) fn resolve_registry_with_dependencies(
    attr_catalog: &mut AttributeCatalog,
    repo: RegistryRepo,
    specs: Vec<SemConvSpecWithProvenance>,
    imports: Vec<ImportsWithProvenance>,
    dependencies: Vec<ResolvedDependency>,
    include_unreferenced: bool,
) -> WResult<Registry, Error> {
    let groups = specs
        .into_iter()
        .map(|g| g.into_v1())
        .flat_map(|SemConvSpecV1WithProvenance { spec, provenance }| {
            spec.groups()
                .iter()
                .map(|group| GroupSpecWithProvenance {
                    spec: group.clone(),
                    provenance: provenance.clone(),
                })
                .collect::<Vec<_>>()
        })
        .map(group_from_spec)
        .collect();
    let mut ureg = UnresolvedRegistry {
        registry: Registry::new(repo.registry_path_repr()),
        groups,
        imports,
        dependencies,
    };

    // Now we do the resolution.
    if let Err(e) = resolve_prefix_on_attributes(&mut ureg) {
        return WResult::FatalErr(e);
    }

    if let Err(e) = resolve_extends_references(&mut ureg) {
        return WResult::FatalErr(e);
    }

    if let Err(e) = resolve_attribute_references(&mut ureg, attr_catalog) {
        return WResult::FatalErr(e);
    }

    // We need to *import* objects from the dependencies as required.
    // If the flag to pull in all dependencies is set, we should grab ALL
    // groups from our dependency.
    if let Err(e) = resolve_dependency_imports(&mut ureg, attr_catalog, include_unreferenced) {
        return WResult::FatalErr(e);
    }

    // Now we do validations.
    let mut errors = vec![];

    // Note: this will remove all the `groups` from UnresolvedRegistry and create
    // a complete `Registry` that is returned.
    //
    // This will also "taint" that attribute catalog so it cannot be used for creating new attribute refs.
    let result = cleanup_and_stabilize_catalog_and_registry(attr_catalog, ureg);
    let attr_name_index = attr_catalog.attribute_name_index();

    // Other complementary checks.
    // Check for duplicate group IDs.
    check_uniqueness(
        &result,
        &mut errors,
        |group| Some(group.id.clone()),
        |group_id, provenances| DuplicateGroupId {
            group_id,
            provenances,
        },
    );
    // Check for duplicate metric names.
    check_uniqueness(
        &result,
        &mut errors,
        |group| group.metric_name.clone(),
        |metric_name, provenances| DuplicateMetricName {
            metric_name,
            provenances,
        },
    );
    // Check for duplicate group names.
    check_uniqueness(
        &result,
        &mut errors,
        |group| group.name.clone(),
        |group_name, provenances| DuplicateGroupName {
            group_name,
            provenances,
        },
    );
    check_root_attribute_id_duplicates(&result, &attr_name_index, &mut errors);

    WResult::OkWithNFEs(result, errors)
}

/// Generic function to check for duplicate keys in the given registry.
///
/// A key can be a group ID, a metric name, an event name, or any other key that is used
/// to identify a group.
fn check_uniqueness<K, KF, EF>(
    registry: &Registry,
    errors: &mut Vec<Error>,
    key_fn: KF,
    error_fn: EF,
) where
    K: Eq + Display + Hash,
    KF: Fn(&Group) -> Option<K>,
    EF: Fn(String, Vec<Provenance>) -> Error,
{
    let mut keys: HashMap<K, Vec<Provenance>> = HashMap::new();

    for group in registry.groups.iter() {
        if let Some(key) = key_fn(group) {
            let provenances = keys.entry(key).or_default();
            provenances.push(group.provenance());
        }
    }

    for (key, provenances) in keys {
        if provenances.len() > 1 {
            // Deduplicate the provenances.
            let provenances: HashSet<Provenance> = provenances.into_iter().unique().collect();

            errors.push(error_fn(key.to_string(), provenances.into_iter().collect()));
        }
    }
}

/// Checks for duplicate attribute IDs in the given registry.
///
/// This function iterates over all groups in the registry that are of type `AttributeGroup`.
/// For each root attribute in these groups (i.e. the ones without lineage), it maps the root
/// attribute ID to the group ID.
/// It then checks if any root attribute ID is found in multiple groups and collects errors
/// for such duplicates.
///
/// # Arguments
///
/// * `registry` - The registry to check for duplicate attribute IDs.
/// * `attr_name_index` - The index of attribute names (catalog).
/// * `errors` - The list of errors to append the duplicate attribute ID errors to.
///
/// # Returns
///
/// This function returns `Ok(())` if no duplicate attribute IDs are found. Otherwise, it returns
/// an error indicating the duplicate attribute IDs.
pub fn check_root_attribute_id_duplicates(
    registry: &Registry,
    attr_name_index: &[String],
    errors: &mut Vec<Error>,
) {
    // Map to track groups by their root attribute ID.
    let mut groups_by_root_attr_id = HashMap::new();
    // Iterate over all groups in the registry that are of type `AttributeGroup`.
    registry
        .groups
        .iter()
        .filter(|group| group.r#type == GroupType::AttributeGroup)
        .for_each(|group| {
            // Iterate over all attribute references in the group.
            for attr_ref in group.attributes.iter() {
                // Get the attribute ID from the attribute name index.
                let attr_id = &attr_name_index[attr_ref.0 as usize];
                // Check if the group has a lineage and if the lineage does not already have the attribute.
                if let Some(lineage) = group.lineage.as_ref() {
                    if !lineage.has_attribute(attr_id) {
                        // Add the group ID to the map entry for the attribute ID.
                        groups_by_root_attr_id
                            .entry(attr_id.clone())
                            .or_insert_with(Vec::new)
                            .push(group.id.clone());
                    }
                }
            }
        });
    // Collect errors for attribute IDs that are found in multiple groups.
    let local_errors: Vec<_> = groups_by_root_attr_id
        .into_iter()
        .filter(|(_, group_ids)| group_ids.len() > 1)
        .map(|(attr_id, group_ids)| Error::DuplicateAttributeId {
            attribute_id: attr_id,
            group_ids,
        })
        .collect();
    errors.extend(local_errors);
}

/// Creates a group from a semantic convention group specification.
/// Note: this function does not resolve references.
fn group_from_spec(group: GroupSpecWithProvenance) -> UnresolvedGroup {
    let attrs = group
        .spec
        .attributes
        .into_iter()
        .map(|attr| UnresolvedAttribute { spec: attr })
        .collect::<Vec<UnresolvedAttribute>>();

    UnresolvedGroup {
        group: Group {
            id: group.spec.id,
            r#type: group.spec.r#type,
            brief: group.spec.brief,
            note: group.spec.note,
            prefix: group.spec.prefix,
            extends: group.spec.extends,
            stability: group.spec.stability,
            deprecated: group.spec.deprecated,
            attributes: vec![],
            span_kind: group.spec.span_kind,
            events: group.spec.events,
            metric_name: group.spec.metric_name,
            instrument: group.spec.instrument,
            unit: group.spec.unit,
            name: group.spec.name,
            lineage: Some(GroupLineage::new(group.provenance.clone())),
            display_name: group.spec.display_name,
            body: group.spec.body,
            annotations: group.spec.annotations,
            entity_associations: group.spec.entity_associations,
            visibility: group.spec.visibility.clone(),
        },
        attributes: attrs,
        provenance: group.provenance,
        include_groups: group.spec.include_groups,
        visibility: group.spec.visibility,
    }
}

/// This takes all attributes and ensures that their id is fully fleshed out with
/// the group prefix before continuing resolution.
///
/// This should be the *only* method that updates attribute ids.
fn resolve_prefix_on_attributes(ureg: &mut UnresolvedRegistry) -> Result<(), Error> {
    for unresolved_group in ureg.groups.iter_mut() {
        if !unresolved_group.group.prefix.is_empty() {
            for attribute in unresolved_group.attributes.iter_mut() {
                if let AttributeSpec::Id { id, .. } = &mut attribute.spec {
                    *id = format!("{}.{}", unresolved_group.group.prefix, id);
                }
            }
        }
    }
    Ok(())
}

/// Resolves imports defined on dependencies.
///
/// If `include_all` is true, then all groups are imported
/// from all dependencies.
fn resolve_dependency_imports(
    ureg: &mut UnresolvedRegistry,
    attribute_catalog: &mut AttributeCatalog,
    include_all: bool,
) -> Result<(), Error> {
    // Import from our dependencies, and add to the final registry.
    let imports = &ureg.imports;
    let dependencies = &ureg.dependencies;
    let groups = dependencies.import_groups(imports, include_all, attribute_catalog)?;
    for group in groups {
        let provenance = group.provenance();
        ureg.groups.push(UnresolvedGroup {
            group,
            attributes: vec![],
            include_groups: vec![],
            visibility: None,
            provenance,
        });
    }
    Ok(())
}

/// Resolves attribute references in the given registry.
/// The resolution process is iterative. The process stops when all the
/// attribute references are resolved or when no attribute reference could
/// be resolved in an iteration.
///
/// The resolve method of the attribute catalog is used to resolve the
/// attribute references.
///
/// Returns true if all the attribute references could be resolved.
fn resolve_attribute_references(
    ureg: &mut UnresolvedRegistry,
    attr_catalog: &mut AttributeCatalog,
) -> Result<(), Error> {
    // TODO - Right now the attribute registry does NOT have any of the
    // attributes from dependencies. We expect to resolve all groups in the current
    // algorithm, instead we need to *pre-register* those attributes here.
    loop {
        let mut errors = vec![];
        let mut resolved_attr_count = 0;

        // Iterate over all groups and resolve the attributes.
        for unresolved_group in ureg.groups.iter_mut() {
            // TODO - we need to look up attributes from dependencies in needed here.
            let mut resolved_attr = vec![];

            // Remove attributes that are resolved and keep unresolved attributes
            // in the group for the next iteration.
            unresolved_group.attributes = unresolved_group
                .attributes
                .clone()
                .into_iter()
                .filter_map(|attr| {
                    let attr_ref = attr_catalog.resolve(
                        &unresolved_group.group.id,
                        &unresolved_group.group.prefix,
                        &attr.spec,
                        unresolved_group.group.lineage.as_mut(),
                        &ureg.dependencies,
                    );
                    if let Some(attr_ref) = attr_ref {
                        // Attribute reference resolved successfully.
                        resolved_attr.push(attr_ref);
                        resolved_attr_count += 1;

                        // Return None to remove this attribute from the
                        // unresolved group.
                        None
                    } else {
                        // Attribute reference could not be resolved.
                        if let AttributeSpec::Ref { r#ref, .. } = &attr.spec {
                            // Keep track of unresolved attribute references in
                            // the errors.
                            errors.push(Error::UnresolvedAttributeRef {
                                group_id: unresolved_group.group.id.clone(),
                                attribute_ref: r#ref.clone(),
                                provenance: unresolved_group.provenance.clone(),
                            });
                        }
                        Some(attr)
                    }
                })
                .collect();

            unresolved_group.group.attributes.extend(resolved_attr);
        }

        if errors.is_empty() {
            break;
        }

        // If we still have unresolved attributes but we did not resolve any
        // attributes in the last iteration, we are stuck in an infinite loop.
        // It means that we have an issue with the semantic convention
        // specifications.
        if resolved_attr_count == 0 {
            return Err(Error::CompoundError(errors));
        }
    }

    Ok(())
}

/// Helper function to add a resolved group to the index and update its state
fn add_resolved_group_to_index(
    group_index: &mut HashMap<String, Vec<UnresolvedAttribute>>,
    unresolved_group: &mut UnresolvedGroup,
    resolved_group_count: &mut usize,
) {
    log::debug!(
        "Adding group {} to index with attribute ids: {:#?}",
        unresolved_group.group.id,
        unresolved_group
            .attributes
            .iter()
            .map(|a| a.spec.id().clone())
            .collect::<Vec<_>>()
    );
    _ = unresolved_group.group.extends.take();
    unresolved_group.include_groups.clear();
    _ = group_index.insert(
        unresolved_group.group.id.clone(),
        unresolved_group.attributes.clone(),
    );
    *resolved_group_count += 1;
}
/// The resolution process is iterative. The process stops when all the
/// `extends` references are resolved or when no `extends` reference could
/// be resolved in an iteration.
///
/// Returns true if all the `extends` references have been resolved.
fn resolve_extends_references(ureg: &mut UnresolvedRegistry) -> Result<(), Error> {
    loop {
        let mut errors = vec![];
        let mut resolved_group_count = 0;

        // Create a map group_id -> attributes for groups
        // that don't have an `extends` clause.
        let mut group_index = HashMap::new();
        let dependencies = &ureg.dependencies;
        // TODO - we need to add in the *dependencies* registry here for lookups.
        for group in ureg.groups.iter() {
            if group.group.extends.is_none() && group.include_groups.is_empty() {
                log::debug!(
                    "Adding group {} to index with attribute ids: {:#?}",
                    group.group.id,
                    group
                        .attributes
                        .iter()
                        .map(|a| a.spec.id().clone())
                        .collect::<Vec<_>>()
                );
                _ = group_index.insert(group.group.id.clone(), group.attributes.clone());
            }
        }
        // Iterate over all groups and resolve the `extends` clauses.
        for unresolved_group in ureg.groups.iter_mut() {
            // TODO - also look in dependencies.
            if let Some(extends) = unresolved_group.group.extends.as_ref() {
                if let Some(attrs) =
                    lookup_group_attributes_with_dependencies(dependencies, &group_index, extends)
                {
                    unresolved_group.attributes = resolve_inheritance_attrs_unified(
                        &unresolved_group.group.id,
                        &unresolved_group.attributes,
                        vec![(extends, &attrs)],
                        unresolved_group.group.lineage.as_mut(),
                    );
                    if let Some(lineage) = unresolved_group.group.lineage.as_mut() {
                        lineage.extends(extends);
                    }
                    add_resolved_group_to_index(
                        &mut group_index,
                        unresolved_group,
                        &mut resolved_group_count,
                    );
                // TODO - first check imports.
                } else {
                    errors.push(Error::UnresolvedExtendsRef {
                        group_id: unresolved_group.group.id.clone(),
                        extends_ref: extends.clone(),
                        provenance: unresolved_group.provenance.clone(),
                    });
                }
            } else if !unresolved_group.include_groups.is_empty() {
                // Iterate over all groups and resolve the `include_groups` clauses.
                let mut attr_ids = HashMap::new();
                let mut attrs_by_group = HashMap::new();
                let mut all_resolved = true;

                for include_group in unresolved_group.include_groups.iter() {
                    if let Some(attrs) = group_index.get(include_group) {
                        // check if any attribute in the attrs is already in the all_attrs
                        // and fail - this is a diamond include problem and is not allowed.
                        // Otherwise add all of them to all_attrs
                        for attr in attrs {
                            if attr_ids.contains_key(&attr.spec.id()) {
                                errors.push(Error::DuplicateAttributeId {
                                    group_ids: unresolved_group.include_groups.clone(),
                                    attribute_id: attr.spec.id().clone(),
                                });
                                all_resolved = false;
                            } else {
                                _ = attr_ids.insert(attr.spec.id().clone(), attr);
                            }
                        }
                        _ = attrs_by_group.insert(include_group.clone(), attrs);

                        // We'll need to reverse engineer if it was a private group later in V2 mapping.
                        if let Some(lineage) = unresolved_group.group.lineage.as_mut() {
                            // update lineage so we know a group was included.
                            lineage.includes_group(include_group);
                        }
                    } else {
                        errors.push(Error::UnresolvedExtendsRef {
                            group_id: unresolved_group.group.id.clone(),
                            extends_ref: include_group.clone(),
                            provenance: unresolved_group.provenance.clone(),
                        });
                        all_resolved = false;
                    }
                }

                if all_resolved {
                    unresolved_group.attributes = resolve_inheritance_attrs_unified(
                        &unresolved_group.group.id,
                        &unresolved_group.attributes,
                        attrs_by_group
                            .iter()
                            .map(|(id, attrs)| (id.as_str(), attrs.as_slice()))
                            .collect(),
                        unresolved_group.group.lineage.as_mut(),
                    );
                    add_resolved_group_to_index(
                        &mut group_index,
                        unresolved_group,
                        &mut resolved_group_count,
                    );
                }
            }
        }

        if errors.is_empty() {
            break;
        }

        log::debug!(
            "Resolved {resolved_group_count} extends in this iteration, found errors {errors:#?}"
        );
        // If we still have unresolved `extends` but we did not resolve any
        // `extends` in the last iteration, we are stuck in an infinite loop.
        // It means that we have an issue with the semantic convention
        // specifications.
        if resolved_group_count == 0 {
            return Err(Error::CompoundError(errors));
        }
    }
    Ok(())
}

fn resolve_inheritance_attrs_unified(
    group_id: &str,
    attrs_group: &[UnresolvedAttribute],
    include_groups: Vec<(&str, &[UnresolvedAttribute])>,
    group_lineage: Option<&mut GroupLineage>,
) -> Vec<UnresolvedAttribute> {
    struct AttrWithLineage {
        spec: AttributeSpec,
        lineage: AttributeLineage,
    }

    // A map attribute_id -> attribute_spec + lineage.
    //
    // Note: we use a BTreeMap to ensure that the attributes are sorted by
    // their id in the resolved registry. This is useful for unit tests to
    // ensure that the resolved registry is easy to compare.
    let mut inherited_attrs = BTreeMap::new();

    // Inherit the attributes from all included groups.
    for (parent_group_id, included_group) in include_groups {
        for parent_attr in included_group.iter() {
            let attr_id = parent_attr.spec.id();
            let lineage = AttributeLineage::inherit_from(parent_group_id, &parent_attr.spec);
            log::debug!(
                "Inheriting attribute {} from group {}, resolved to {:#?}",
                attr_id,
                parent_group_id,
                lineage.source_group
            );
            _ = inherited_attrs.insert(
                attr_id.clone(),
                AttrWithLineage {
                    spec: parent_attr.spec.clone(),
                    lineage,
                },
            );
        }
    }

    // Override the inherited attributes with the attributes from the group.
    for attr in attrs_group.iter() {
        match &attr.spec {
            AttributeSpec::Ref { r#ref, .. } => {
                if let Some(AttrWithLineage {
                    spec: parent_attr,
                    lineage,
                }) = inherited_attrs.get_mut(r#ref)
                {
                    *parent_attr = resolve_inheritance_attr(&attr.spec, parent_attr, lineage);
                } else {
                    _ = inherited_attrs.insert(
                        r#ref.clone(),
                        AttrWithLineage {
                            spec: attr.spec.clone(),
                            lineage: AttributeLineage::new(group_id),
                        },
                    );
                }
            }
            AttributeSpec::Id { id, .. } => {
                _ = inherited_attrs.insert(
                    id.clone(),
                    AttrWithLineage {
                        spec: attr.spec.clone(),
                        lineage: AttributeLineage::new(group_id),
                    },
                );
            }
        }
    }

    let inherited_attrs = inherited_attrs.into_values();
    if let Some(group_lineage) = group_lineage {
        inherited_attrs
            .map(|attr_with_lineage| {
                if !attr_with_lineage.lineage.is_empty() {
                    group_lineage.add_attribute_lineage(
                        attr_with_lineage.spec.id(),
                        attr_with_lineage.lineage,
                    );
                }
                UnresolvedAttribute {
                    spec: attr_with_lineage.spec,
                }
            })
            .collect()
    } else {
        inherited_attrs
            .map(|attr_with_lineage| UnresolvedAttribute {
                spec: attr_with_lineage.spec,
            })
            .collect()
    }
}

fn resolve_inheritance_attr(
    attr: &AttributeSpec,
    parent_attr: &AttributeSpec,
    lineage: &mut AttributeLineage,
) -> AttributeSpec {
    match attr {
        AttributeSpec::Ref {
            r#ref,
            brief,
            examples,
            tag,
            requirement_level,
            sampling_relevant,
            note,
            stability,
            deprecated,
            prefix,
            annotations,
            role,
        } => {
            match parent_attr {
                AttributeSpec::Ref {
                    brief: parent_brief,
                    examples: parent_examples,
                    tag: parent_tag,
                    requirement_level: parent_requirement_level,
                    sampling_relevant: parent_sampling_relevant,
                    note: parent_note,
                    stability: parent_stability,
                    deprecated: parent_deprecated,
                    prefix: parent_prefix,
                    annotations: parent_annotations,
                    role: parent_role,
                    ..
                } => {
                    // attr and attr_parent are both references.
                    AttributeSpec::Ref {
                        r#ref: r#ref.clone(),
                        brief: lineage.optional_brief(brief, parent_brief),
                        examples: lineage.examples(examples, parent_examples),
                        tag: lineage.tag(tag, parent_tag),
                        requirement_level: lineage.optional_requirement_level(
                            requirement_level,
                            parent_requirement_level,
                        ),
                        sampling_relevant: lineage
                            .sampling_relevant(sampling_relevant, parent_sampling_relevant),
                        note: lineage.optional_note(note, parent_note),
                        stability: lineage.stability(stability, parent_stability),
                        deprecated: lineage.deprecated(deprecated, parent_deprecated),
                        prefix: lineage.prefix(prefix, parent_prefix),
                        annotations: lineage.annotations(annotations, parent_annotations),
                        role: lineage.optional_role(role, parent_role),
                    }
                }
                AttributeSpec::Id {
                    r#type: parent_type,
                    brief: parent_brief,
                    examples: parent_examples,
                    tag: parent_tag,
                    requirement_level: parent_requirement_level,
                    sampling_relevant: parent_sampling_relevant,
                    note: parent_note,
                    stability: parent_stability,
                    deprecated: parent_deprecated,
                    annotations: parent_annotations,
                    role: parent_role,
                    ..
                } => {
                    // attr is a reference and attr_parent is an id.
                    // We need to override the reference with the id.
                    AttributeSpec::Id {
                        id: r#ref.clone(),
                        r#type: parent_type.clone(),
                        brief: lineage.optional_brief(brief, parent_brief),
                        examples: lineage.examples(examples, parent_examples),
                        tag: lineage.tag(tag, parent_tag),
                        requirement_level: lineage
                            .requirement_level(requirement_level, parent_requirement_level),
                        sampling_relevant: lineage
                            .sampling_relevant(sampling_relevant, parent_sampling_relevant),
                        note: lineage.note(note, parent_note),
                        stability: lineage.stability(stability, parent_stability),
                        deprecated: lineage.deprecated(deprecated, parent_deprecated),
                        annotations: lineage.annotations(annotations, parent_annotations),
                        role: lineage.optional_role(role, parent_role),
                    }
                }
            }
        }
        AttributeSpec::Id { .. } => attr.clone(),
    }
}

// Note The below helper functions are to avoid causing RUST
// to be confused about borrowing mutable aspects of the Unresolved Registry.
//
// We borrow dependencies as immutable, always, so this should be safe.
// We do NOT borrow the index as mutable when iterating over groups, but
// Rust's type system is not advanced enough to know about partial mutable
// borrowing of a reference.
fn lookup_group_attributes_with_dependencies(
    dependencies: &[ResolvedDependency],
    local_index: &HashMap<String, Vec<UnresolvedAttribute>>,
    id: &str,
) -> Option<Vec<UnresolvedAttribute>> {
    // First check our direct groups.
    if let Some(attrs) = local_index.get(id) {
        return Some(attrs.clone());
    }
    // Now check dependencies in order.
    dependencies
        .iter()
        .find_map(|d| d.lookup_group_attributes(id))
}

/// This will sort the clean and sort the attribute catalog and registry.
///
/// This helps with idempotent/stable resolved schemas.
pub(crate) fn cleanup_and_stabilize_catalog_and_registry(
    attr_catalog: &mut AttributeCatalog,
    mut ureg: UnresolvedRegistry,
) -> Registry {
    // Clean up the attribute registry and groups to have consistent ordering.
    let attr_refs: HashSet<AttributeRef> = ureg
        .groups
        .iter()
        .flat_map(|g| g.group.attributes.iter().cloned())
        .collect();
    let mapping = attr_catalog.gc_unreferenced_attribute_refs_and_sort(attr_refs);
    for g in ureg.groups.iter_mut() {
        for a in g.group.attributes.iter_mut() {
            if let Some(ar) = mapping.get(a) {
                *a = *ar;
            }
        }
    }

    // Sort groups by id.
    ureg.groups.sort_by(|l, r| l.group.id.cmp(&r.group.id));

    // Sort the attribute internal references in each group.
    // This is needed to ensure that the resolved registry is easy to compare
    // in unit tests.
    ureg.registry.groups = ureg
        .groups
        .into_iter()
        .filter(|g| g.visibility != Some(AttributeGroupVisibilitySpec::Internal))
        .map(|mut g| {
            g.group.attributes.sort();
            g.group
        })
        .collect();
    ureg.registry
}

#[cfg(test)]
mod tests {
    use rand::rng;
    use rand::seq::SliceRandom;
    use std::cmp::Ordering;
    use std::collections::HashMap;
    use std::error::Error;
    use std::fs::OpenOptions;
    use std::path::PathBuf;

    use glob::glob;
    use serde::Serialize;
    use weaver_common::result::WResult;
    use weaver_common::vdir::VirtualDirectoryPath;
    use weaver_diff::canonicalize_json_string;
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_resolved_schema::registry::Group;
    use weaver_resolved_schema::registry::Registry;
    use weaver_semconv::group::GroupType;
    use weaver_semconv::provenance::Provenance;
    use weaver_semconv::registry_repo::RegistryRepo;

    use crate::attribute::AttributeCatalog;
    use crate::registry::cleanup_and_stabilize_catalog_and_registry;
    use crate::registry::resolve_registry_with_dependencies;
    use crate::registry::UnresolvedGroup;
    use crate::registry::UnresolvedRegistry;
    use crate::LoadedSemconvRegistry;
    use crate::SchemaResolver;

    /// Test the resolution of semantic convention registries stored in the
    /// data directory. The provided test cases cover the following resolution
    /// scenarios:
    /// - Attribute references.
    /// - Extends references.
    ///
    /// Each test is stored in a directory named `registry-test-*` and contains
    /// the following directory and files:
    /// - directory `registry` containing the semantic convention specifications
    ///   in YAML format.
    /// - file `expected-attribute-catalog.json` containing the expected
    ///   attribute catalog in JSON format.
    /// - file `expected-registry.json` containing the expected registry in
    ///   JSON format.
    #[test]
    #[allow(clippy::print_stdout)]
    fn test_registry_resolution() {
        let skip_tests: Vec<&str> = vec![
            // "registry-test-10-prefix-refs",
            // "registry-test-11-prefix-refs-extends",
            // "registry-test-3-extends",
            // "registry-test-4-events",
            // "registry-test-6-resources",
            // "registry-test-7-spans",
            // "registry-test-8-http",
            // "registry-test-v2-2-multifile",
        ];
        // Iterate over all directories in the data directory and
        // starting with registry-test-*
        for test_entry in glob("data/registry-test-*").expect("Failed to read glob pattern") {
            let path_buf = test_entry.expect("Failed to read test directory");
            let test_dir = path_buf
                .to_str()
                .expect("Failed to convert test directory to string");

            if skip_tests.iter().any(|skip| test_dir.ends_with(skip)) {
                // Skip the test for now as it is not yet supported.
                continue;
            }
            println!("Testing `{test_dir}`");

            // Delete all the files in the observed_output/target directory
            // before generating the new files.
            std::fs::remove_dir_all(format!("observed_output/{test_dir}")).unwrap_or_default();
            let observed_output_dir = PathBuf::from(format!("observed_output/{test_dir}"));
            std::fs::create_dir_all(observed_output_dir.clone())
                .expect("Failed to create observed output directory");
            let registry_id = "default";
            let location: VirtualDirectoryPath = format!("{test_dir}/registry")
                .try_into()
                .expect("Failed to parse file directory");
            let loaded = SchemaResolver::load_semconv_repository(
                RegistryRepo::try_new(registry_id, &location).expect("Failed to load registry"),
                true,
            )
            .ignore(|e| {
                // Ignore prefix errors on tests of prefix.
                test_dir.contains("prefix")
                    && matches!(
                        e,
                        weaver_semconv::Error::InvalidGroupUsesPrefix {
                            path_or_url: _,
                            group_id: _
                        }
                    )
            })
            .ignore(|e| {
                matches!(
                    e,
                    weaver_semconv::Error::UnstableFileVersion {
                        file_format: _,
                        provenance: _,
                    }
                )
            })
            .into_result_failing_non_fatal()
            .expect("Failed to load semconv specs");

            let LoadedSemconvRegistry::Unresolved {
                repo,
                specs,
                imports,
                dependencies,
            } = loaded
            else {
                panic!("Should have loaded an unresolved registry")
            };
            assert!(
                dependencies.is_empty(),
                "Unable to handle dependencies in resolution unit tests"
            );
            let mut attr_catalog = AttributeCatalog::default();
            let observed_registry = resolve_registry_with_dependencies(
                &mut attr_catalog,
                repo,
                specs,
                imports,
                vec![],
                false,
            )
            .into_result_failing_non_fatal();
            // Check that the resolved attribute catalog matches the expected attribute catalog.
            let observed_attr_catalog = attr_catalog.drain_attributes();

            // Check presence of an `expected-errors.json` file.
            // If the file is present, the test is expected to fail with the errors in the file.
            let expected_errors_file = format!("{test_dir}/expected-errors.json");
            if PathBuf::from(&expected_errors_file).exists() {
                assert!(observed_registry.is_err(), "This test is expected to fail");
                let expected_errors: String = std::fs::read_to_string(&expected_errors_file)
                    .expect("Failed to read expected errors file");
                let observed_errors = serde_json::to_string(&observed_registry).unwrap();
                // TODO - Write observed errors.

                assert_eq!(
                    canonicalize_json_string(&observed_errors).unwrap(),
                    canonicalize_json_string(&expected_errors).unwrap(),
                    "Observed and expected errors don't match for `{}`.\n{}",
                    test_dir,
                    weaver_diff::diff_output(&expected_errors, &observed_errors)
                );
                continue;
            }

            // At this point, the normal behavior of this test is to pass.
            let mut observed_registry = observed_registry.expect("Failed to resolve the registry");
            // Force registry URL to consistent string
            observed_registry.registry_url = "https://127.0.0.1".to_owned();
            // Now sort groups so we don't get flaky tests.
            observed_registry.groups.sort_by_key(|g| g.id.to_owned());

            // Load the expected registry and attribute catalog.
            let expected_attr_catalog_file = format!("{test_dir}/expected-attribute-catalog.json");
            let expected_attr_catalog: Vec<Attribute> = serde_json::from_reader(
                std::fs::File::open(expected_attr_catalog_file)
                    .expect("Failed to open expected attribute catalog"),
            )
            .expect("Failed to deserialize expected attribute catalog");

            // Write observed output.
            let observed_attr_catalog_file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(observed_output_dir.join("attribute-catalog.json"))
                .expect("Failed to open observed output file");
            serde_json::to_writer_pretty(observed_attr_catalog_file, &observed_attr_catalog)
                .expect("Failed to write observed output.");
            // Compare values
            assert_eq!(
                observed_attr_catalog, expected_attr_catalog,
                "Observed and expected attribute catalogs don't match for `{}`.\nDiff from expected:\n{}",
                test_dir, weaver_diff::diff_output(&to_json(&expected_attr_catalog), &to_json(&observed_attr_catalog))
            );

            // Check that the resolved registry matches the expected registry.
            let expected_registry: Registry = serde_json::from_reader(
                std::fs::File::open(format!("{test_dir}/expected-registry.json"))
                    .expect("Failed to open expected registry"),
            )
            .expect("Failed to deserialize expected registry");

            // Write observed output.
            let observed_registry_file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(observed_output_dir.join("registry.json"))
                .expect("Failed to open observed output file");
            serde_json::to_writer_pretty(observed_registry_file, &observed_registry)
                .expect("Failed to write observed output.");

            assert_eq!(
                observed_registry,
                expected_registry,
                "Expected and observed registry don't match for `{}`.\nDiff from expected:\n{}",
                test_dir,
                weaver_diff::diff_output(
                    &to_json(&expected_registry),
                    &to_json(&observed_registry)
                )
            );

            // let yaml = serde_yaml::to_string(&observed_registry).unwrap();
            // println!("{}", yaml);
        }
    }

    fn create_registry_from_string(registry_spec: &str) -> WResult<Registry, crate::Error> {
        let loaded = LoadedSemconvRegistry::create_from_string(registry_spec)
            .expect("Failed to load semconv spec");
        SchemaResolver::resolve(loaded, false).map(|schema| schema.registry)
    }

    #[test]
    fn test_registry_error_unresolved_extends() {
        let result = create_registry_from_string(
            "
groups:
    - id: group.one
      type: attribute_group
      brief: \"Group one\"
      extends: group.non.existent.one
    - id: group.two
      type: attribute_group
      brief: \"Group two\"
      extends: group.non.existent.two",
        )
        .into_result_failing_non_fatal();

        assert!(result.is_err());

        if let crate::Error::CompoundError(errors) = result.unwrap_err() {
            assert!(errors.len() == 2);
        } else {
            panic!("Expected a CompoundError");
        }
    }

    #[test]
    fn test_registry_error_unresolved_refs() {
        let result = create_registry_from_string(
            "
groups:
    - id: span.one
      type: span
      span_kind: internal
      stability: stable
      brief: 'Span one'
      attributes:
        - ref: non.existent.one
          requirement_level: opt_in
        - ref: non.existent.two
          requirement_level: opt_in",
        )
        .into_result_failing_non_fatal();

        assert!(result.is_err());

        if let crate::Error::CompoundError(errors) = result.unwrap_err() {
            assert!(errors.len() == 2);
        } else {
            panic!("Expected a CompoundError");
        }
    }

    #[test]
    fn test_api_usage() -> Result<(), Box<dyn Error>> {
        let registry_id = "local";

        // Load a semantic convention registry from a local directory.
        // Note: A method is also available to load a registry from a git
        // repository.
        // TODO - registry path.
        let path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-7-spans/registry".to_owned(),
        };
        let repo = RegistryRepo::try_new(registry_id, &path)?;
        let loaded =
            SchemaResolver::load_semconv_repository(repo, true).into_result_failing_non_fatal()?;
        let resolved_schema =
            SchemaResolver::resolve(loaded, false).into_result_failing_non_fatal()?;

        // Get the resolved registry by its ID.
        let resolved_registry = &resolved_schema.registry;

        // Get the catalog of the resolved telemetry schema.
        let catalog = resolved_schema.catalog();
        // Scan over all the metrics
        let mut metric_count = 0;
        for metric in resolved_registry.groups(GroupType::Metric) {
            metric_count += 1;
            let _resolved_attributes = metric.attributes(catalog)?;
            // Do something with the resolved attributes.
        }
        assert_eq!(
            metric_count, 0,
            "No metric in the resolved registry expected"
        );

        // Scan over all the spans
        let mut span_count = 0;
        for span in resolved_registry.groups(GroupType::Span) {
            span_count += 1;
            let _resolved_attributes = span.attributes(catalog)?;
            // Do something with the resolved attributes.
        }
        assert_eq!(span_count, 10, "10 spans in the resolved registry expected");

        Ok(())
    }

    #[test]
    fn test_drop_unused_attributes_in_catalog() -> Result<(), Box<dyn Error>> {
        let mut catalog = AttributeCatalog::default();
        // Create 26 attribute refs, and then randomize them.
        let mut attr_refs = vec![];
        for c in 'a'..='z' {
            attr_refs.push(catalog.attribute_ref(Attribute {
                name: format!("{c}"),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: Default::default(),
                examples: Default::default(),
                tag: Default::default(),
                requirement_level: Default::default(),
                sampling_relevant: Default::default(),
                note: Default::default(),
                stability: Default::default(),
                deprecated: Default::default(),
                prefix: Default::default(),
                tags: Default::default(),
                annotations: Default::default(),
                value: Default::default(),
                role: Default::default(),
            }));
        }

        // We only need to file out portions here.
        let ureg = UnresolvedRegistry {
            registry: Registry {
                registry_url: "test".to_owned(),
                groups: vec![],
            },
            groups: vec![UnresolvedGroup {
                group: Group {
                    id: "b".to_owned(),
                    r#type: GroupType::AttributeGroup,
                    brief: Default::default(),
                    note: Default::default(),
                    prefix: Default::default(),
                    extends: Default::default(),
                    stability: Default::default(),
                    deprecated: Default::default(),
                    attributes: attr_refs.iter().take(10).cloned().collect(),
                    span_kind: Default::default(),
                    events: Default::default(),
                    metric_name: Default::default(),
                    instrument: Default::default(),
                    unit: Default::default(),
                    name: Default::default(),
                    lineage: Default::default(),
                    display_name: Default::default(),
                    body: Default::default(),
                    annotations: Default::default(),
                    entity_associations: Default::default(),
                    visibility: Default::default(),
                },
                attributes: Default::default(),
                include_groups: Default::default(),
                visibility: Default::default(),
                provenance: Provenance {
                    registry_id: Default::default(),
                    path: Default::default(),
                },
            }],
            imports: vec![],
            dependencies: vec![],
        };

        let _ = cleanup_and_stabilize_catalog_and_registry(&mut catalog, ureg);
        let attrs = catalog.drain_attributes();

        // We should only have 10 attributes.
        assert_eq!(attrs.len(), 10);

        // Check catalog for sorting.
        for (a, expected) in attrs.iter().zip('a'..='z') {
            assert_eq!(a.name, format!("{expected}"));
        }
        Ok(())
    }

    #[test]
    fn test_sort_result_catalog() -> Result<(), Box<dyn Error>> {
        let mut catalog = AttributeCatalog::default();
        // Create 26 attribute refs, and then randomize them.
        let mut attr_refs = vec![];
        for c in 'a'..='z' {
            attr_refs.push(catalog.attribute_ref(Attribute {
                name: format!("{c}"),
                r#type: weaver_semconv::attribute::AttributeType::PrimitiveOrArray(
                    weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
                ),
                brief: Default::default(),
                examples: Default::default(),
                tag: Default::default(),
                requirement_level: Default::default(),
                sampling_relevant: Default::default(),
                note: Default::default(),
                stability: Default::default(),
                deprecated: Default::default(),
                prefix: Default::default(),
                tags: Default::default(),
                annotations: Default::default(),
                value: Default::default(),
                role: Default::default(),
            }));
        }
        // 2. Get a thread-local random number generator (RNG)
        let mut rng = rng();
        attr_refs.shuffle(&mut rng);

        // We only need to fill out the registry here.
        let ureg = UnresolvedRegistry {
            registry: Registry {
                registry_url: "test".to_owned(),
                groups: vec![],
            },
            groups: vec![
                UnresolvedGroup {
                    group: Group {
                        id: "b".to_owned(),
                        r#type: GroupType::AttributeGroup,
                        brief: Default::default(),
                        note: Default::default(),
                        prefix: Default::default(),
                        extends: Default::default(),
                        stability: Default::default(),
                        deprecated: Default::default(),
                        attributes: attr_refs.iter().take(10).cloned().collect(),
                        span_kind: Default::default(),
                        events: Default::default(),
                        metric_name: Default::default(),
                        instrument: Default::default(),
                        unit: Default::default(),
                        name: Default::default(),
                        lineage: Default::default(),
                        display_name: Default::default(),
                        body: Default::default(),
                        annotations: Default::default(),
                        entity_associations: Default::default(),
                        visibility: Default::default(),
                    },
                    attributes: Default::default(),
                    include_groups: Default::default(),
                    visibility: Default::default(),
                    provenance: Provenance {
                        registry_id: Default::default(),
                        path: Default::default(),
                    },
                },
                UnresolvedGroup {
                    group: Group {
                        id: "a".to_owned(),
                        r#type: GroupType::AttributeGroup,
                        brief: Default::default(),
                        note: Default::default(),
                        prefix: Default::default(),
                        extends: Default::default(),
                        stability: Default::default(),
                        deprecated: Default::default(),
                        attributes: attr_refs.iter().skip(10).take(10).cloned().collect(),
                        span_kind: Default::default(),
                        events: Default::default(),
                        metric_name: Default::default(),
                        instrument: Default::default(),
                        unit: Default::default(),
                        name: Default::default(),
                        lineage: Default::default(),
                        display_name: Default::default(),
                        body: Default::default(),
                        annotations: Default::default(),
                        entity_associations: Default::default(),
                        visibility: Default::default(),
                    },
                    attributes: Default::default(),
                    include_groups: Default::default(),
                    visibility: Default::default(),
                    provenance: Provenance {
                        registry_id: Default::default(),
                        path: Default::default(),
                    },
                },
                UnresolvedGroup {
                    group: Group {
                        id: "c".to_owned(),
                        r#type: GroupType::AttributeGroup,
                        brief: Default::default(),
                        note: Default::default(),
                        prefix: Default::default(),
                        extends: Default::default(),
                        stability: Default::default(),
                        deprecated: Default::default(),
                        attributes: attr_refs.iter().skip(20).take(6).cloned().collect(),
                        span_kind: Default::default(),
                        events: Default::default(),
                        metric_name: Default::default(),
                        instrument: Default::default(),
                        unit: Default::default(),
                        name: Default::default(),
                        lineage: Default::default(),
                        display_name: Default::default(),
                        body: Default::default(),
                        annotations: Default::default(),
                        entity_associations: Default::default(),
                        visibility: Default::default(),
                    },
                    attributes: Default::default(),
                    include_groups: Default::default(),
                    visibility: Default::default(),
                    provenance: Provenance {
                        registry_id: Default::default(),
                        path: Default::default(),
                    },
                },
            ],
            imports: vec![],
            dependencies: vec![],
        };
        // Group id to expected attribute names.
        let lookups: HashMap<String, Vec<String>> = ureg
            .groups
            .iter()
            .map(|g| {
                let attr_names: Vec<String> = g
                    .group
                    .attributes
                    .iter()
                    .filter_map(|ar| catalog.attribute(ar))
                    .map(|a| a.name.clone())
                    .collect();
                (g.group.id.clone(), attr_names)
            })
            .collect();
        let registry = cleanup_and_stabilize_catalog_and_registry(&mut catalog, ureg);
        let attrs = catalog.drain_attributes();

        // Check catalog for sorting.
        for (a, expected) in attrs.iter().zip('a'..='z') {
            assert_eq!(a.name, format!("{expected}"));
        }
        // Now check registry for sorting.
        for g in &registry.groups {
            for (l, r) in g.attributes.iter().zip(g.attributes.iter().skip(1)) {
                assert_eq!(l.0.cmp(&r.0), Ordering::Less);
            }
            let expected_names = lookups
                .get(&g.id)
                .expect("Expected to find same group output as input");
            assert_eq!(
                expected_names.len(),
                g.attributes.len(),
                "Wrong number of attributes for group: {}",
                g.id
            );
            for ar in &g.attributes {
                let a = &attrs[ar.0 as usize];
                assert!(expected_names.contains(&a.name));
            }
        }
        Ok(())
    }

    fn to_json<T: Serialize + ?Sized>(value: &T) -> String {
        serde_json::to_string_pretty(value).unwrap()
    }
}
