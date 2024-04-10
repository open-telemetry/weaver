// SPDX-License-Identifier: Apache-2.0

//! Functions to resolve a semantic convention registry.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Deserialize;

use weaver_resolved_schema::attribute::UnresolvedAttribute;
use weaver_resolved_schema::lineage::{AttributeLineage, GroupLineage};
use weaver_resolved_schema::registry::{Constraint, Group, Registry};
use weaver_semconv::attribute::AttributeSpec;
use weaver_semconv::{GroupSpecWithProvenance, SemConvRegistry};

use crate::attribute::AttributeCatalog;
use crate::constraint::resolve_constraints;
use crate::{handle_errors, Error, UnsatisfiedAnyOfConstraint};

/// A registry containing unresolved groups.
#[derive(Debug, Deserialize)]
pub struct UnresolvedRegistry {
    /// The semantic convention registry containing resolved groups.
    pub registry: Registry,

    /// List of unresolved groups that belong to the registry.
    /// The resolution process will progressively move the unresolved groups
    /// into the registry field once they are resolved.
    pub groups: Vec<UnresolvedGroup>,
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

    /// The provenance of the group (URL or path).
    pub provenance: String,
}

/// Resolves the semantic convention registry passed as argument and returns
/// the resolved registry or an error if the resolution process failed.
///
/// The resolution process consists of the following steps:
/// - Resolve all attribute references and apply the overrides when needed.
/// - Resolve all the `extends` references.
/// - Resolve all the `include` constraints (i.e. inherit required attributes
///   and any new `any_of` constraints).
/// - Check the `any_of` constraints and return an error if the constraints
///   are not satisfied.
///
/// # Arguments
///
/// * `attr_catalog` - The attribute catalog to use to resolve the attribute references.
/// * `registry_url` - The URL of the registry.
/// * `registry` - The semantic convention registry.
///
/// # Returns
///
/// This function returns the resolved registry or an error if the resolution process
/// failed.
pub fn resolve_semconv_registry(
    attr_catalog: &mut AttributeCatalog,
    registry_url: &str,
    registry: &SemConvRegistry,
) -> Result<Registry, Error> {
    let mut ureg = unresolved_registry_from_specs(registry_url, registry);

    resolve_extends_references(&mut ureg)?;

    resolve_attribute_references(&mut ureg, attr_catalog)?;

    resolve_include_constraints(&mut ureg)?;

    // Sort the attribute internal references in each group.
    // This is needed to ensure that the resolved registry is easy to compare
    // in unit tests.
    ureg.registry.groups = ureg
        .groups
        .into_iter()
        .map(|mut g| {
            g.group.attributes.sort();
            g.group
        })
        .collect();

    // Check the `any_of` constraints.
    let attr_name_index = attr_catalog.attribute_name_index();
    check_any_of_constraints(&ureg.registry, &attr_name_index)?;

    // All constraints are satisfied.
    // Remove the constraints from the resolved registry.
    for group in ureg.registry.groups.iter_mut() {
        group.constraints.clear();
    }

    Ok(ureg.registry)
}

/// Checks the `any_of` constraints in the given registry.
///
/// # Arguments
///
/// * `registry` - The registry to check.
/// * `attr_name_index` - The index of attribute names (catalog).
///
/// # Returns
///
/// This function returns `Ok(())` if all the `any_of` constraints are satisfied.
/// Otherwise, it returns the error `Error::UnsatisfiedAnyOfConstraint`.
pub fn check_any_of_constraints(
    registry: &Registry,
    attr_name_index: &[String],
) -> Result<(), Error> {
    let mut errors = vec![];

    for group in registry.groups.iter() {
        // Build a list of attribute names for the group.
        let mut group_attr_names = HashSet::new();
        for attr_ref in group.attributes.iter() {
            match attr_name_index.get(attr_ref.0 as usize) {
                None => errors.push(Error::UnresolvedAttributeRef {
                    group_id: group.id.clone(),
                    attribute_ref: attr_ref.0.to_string(),
                    provenance: group.provenance().to_owned(),
                }),
                Some(attr_name) => {
                    _ = group_attr_names.insert(attr_name.clone());
                }
            }
        }

        if let Err(e) = check_group_any_of_constraints(
            group.id.as_ref(),
            group_attr_names,
            group.constraints.as_ref(),
        ) {
            errors.push(e);
        }
    }

    handle_errors(errors)?;
    Ok(())
}

/// Checks the `any_of` constraints for the given group.
fn check_group_any_of_constraints(
    group_id: &str,
    group_attr_names: HashSet<String>,
    constraints: &[Constraint],
) -> Result<(), Error> {
    let mut unsatisfied_any_of_constraints: HashMap<&Constraint, UnsatisfiedAnyOfConstraint> =
        HashMap::new();

    for constraint in constraints.iter() {
        if constraint.any_of.is_empty() {
            continue;
        }

        // Check if the group satisfies the `any_of` constraint.
        if let Some(attr) = constraint
            .any_of
            .iter()
            .find(|name| !group_attr_names.contains(*name))
        {
            // The any_of constraint is not satisfied.
            // Insert the attribute into the list of missing attributes for the
            // constraint.
            unsatisfied_any_of_constraints
                .entry(constraint)
                .or_insert_with(|| UnsatisfiedAnyOfConstraint {
                    any_of: constraint.clone(),
                    missing_attributes: vec![],
                })
                .missing_attributes
                .push(attr.clone());
        }
    }
    if !unsatisfied_any_of_constraints.is_empty() {
        let errors = unsatisfied_any_of_constraints
            .into_values()
            .map(|v| Error::UnsatisfiedAnyOfConstraint {
                group_id: group_id.to_owned(),
                any_of: v.any_of,
                missing_attributes: v.missing_attributes,
            })
            .collect();
        return Err(Error::CompoundError(errors));
    }
    Ok(())
}

/// Creates a semantic convention registry from a set of semantic convention
/// specifications.
///
/// This function creates an unresolved registry from the given semantic
/// convention specifications and registry url.
///
/// Note: this function does not resolve references.
///
/// # Arguments
///
/// * `registry_url` - The URL of the registry.
/// * `registry` - The semantic convention specifications.
///
/// # Returns
///
/// This function returns an unresolved registry containing the semantic
/// convention specifications.
fn unresolved_registry_from_specs(
    registry_url: &str,
    registry: &SemConvRegistry,
) -> UnresolvedRegistry {
    let groups = registry
        .groups_with_provenance()
        .map(group_from_spec)
        .collect();

    UnresolvedRegistry {
        registry: Registry {
            registry_url: registry_url.to_owned(),
            groups: vec![],
        },
        groups,
    }
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
            constraints: resolve_constraints(&group.spec.constraints),
            attributes: vec![],
            span_kind: group.spec.span_kind,
            events: group.spec.events,
            metric_name: group.spec.metric_name,
            instrument: group.spec.instrument,
            unit: group.spec.unit,
            name: group.spec.name,
            lineage: Some(GroupLineage::new(group.provenance.clone())),
        },
        attributes: attrs,
        provenance: group.provenance,
    }
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
    loop {
        let mut errors = vec![];
        let mut resolved_attr_count = 0;

        // Iterate over all groups and resolve the attributes.
        for unresolved_group in ureg.groups.iter_mut() {
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
                    );
                    if let Some(attr_ref) = attr_ref {
                        resolved_attr.push(attr_ref);
                        resolved_attr_count += 1;
                        None
                    } else {
                        if let AttributeSpec::Ref { r#ref, .. } = &attr.spec {
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

/// Resolves the `extends` references in the given registry.
/// The resolution process is iterative. The process stops when all the
/// `extends` references are resolved or when no `extends` reference could
/// be resolved in an iteration.
///
/// Returns true if all the `extends` references have been resolved.
fn resolve_extends_references(ureg: &mut UnresolvedRegistry) -> Result<(), Error> {
    loop {
        let mut errors = vec![];
        let mut resolved_extends_count = 0;

        // Create a map group_id -> attributes for groups
        // that don't have an `extends` clause.
        let mut group_index = HashMap::new();
        for group in ureg.groups.iter() {
            if group.group.extends.is_none() {
                _ = group_index.insert(group.group.id.clone(), group.attributes.clone());
            }
        }

        // Iterate over all groups and resolve the `extends` clauses.
        for unresolved_group in ureg.groups.iter_mut() {
            if let Some(extends) = unresolved_group.group.extends.as_ref() {
                if let Some(attrs) = group_index.get(extends) {
                    unresolved_group.attributes = resolve_inheritance_attrs(
                        &unresolved_group.group.id,
                        &unresolved_group.attributes,
                        extends,
                        attrs,
                        unresolved_group.group.lineage.as_mut(),
                    );
                    _ = unresolved_group.group.extends.take();
                    _ = group_index.insert(
                        unresolved_group.group.id.clone(),
                        unresolved_group.attributes.clone(),
                    );
                    resolved_extends_count += 1;
                } else {
                    errors.push(Error::UnresolvedExtendsRef {
                        group_id: unresolved_group.group.id.clone(),
                        extends_ref: unresolved_group
                            .group
                            .extends
                            .clone()
                            .unwrap_or("".to_owned()),
                        provenance: unresolved_group.provenance.clone(),
                    });
                }
            }
        }

        if errors.is_empty() {
            break;
        }
        // If we still have unresolved `extends` but we did not resolve any
        // `extends` in the last iteration, we are stuck in an infinite loop.
        // It means that we have an issue with the semantic convention
        // specifications.
        if resolved_extends_count == 0 {
            return Err(Error::CompoundError(errors));
        }
    }
    Ok(())
}

/// Resolves the `include` constraints in the given registry.
///
/// Possible optimization: the current resolution process is a based on a naive
/// and iterative algorithm that is most likely good enough for now. If the
/// semconv registry becomes too large, we may need to revisit the resolution
/// process to make it more efficient by using a topological sort algorithm.
fn resolve_include_constraints(ureg: &mut UnresolvedRegistry) -> Result<(), Error> {
    loop {
        let mut errors = vec![];
        let mut resolved_include_count = 0;

        // Create a map group_id -> vector of attribute ref for groups
        // that don't have an `include` clause.
        let mut group_attrs_index = HashMap::new();
        let mut group_any_of_index = HashMap::new();
        for group in ureg.groups.iter() {
            if !group.group.has_include() {
                _ = group_attrs_index
                    .insert(group.group.id.clone(), group.group.attributes.clone());
                _ = group_any_of_index.insert(
                    group.group.id.clone(),
                    group
                        .group
                        .constraints
                        .iter()
                        .filter_map(|c| {
                            if c.any_of.is_empty() {
                                None
                            } else {
                                let mut any_of = c.clone();
                                _ = any_of.include.take();
                                Some(any_of)
                            }
                        })
                        .collect::<Vec<Constraint>>(),
                );
            }
        }

        // Iterate over all groups and resolve the `include` constraints.
        for unresolved_group in ureg.groups.iter_mut() {
            let mut attributes_to_import = vec![];
            let mut any_of_to_import = vec![];
            let mut resolved_includes = HashSet::new();

            for constraint in unresolved_group.group.constraints.iter() {
                if let Some(include) = &constraint.include {
                    if let Some(attributes) = group_attrs_index.get(include) {
                        attributes_to_import.extend(attributes.iter().cloned());
                        _ = resolved_includes.insert(include.clone());

                        if let Some(any_of_constraints) = group_any_of_index.get(include) {
                            any_of_to_import.extend(any_of_constraints.iter().cloned());
                        }

                        resolved_include_count += 1;
                    } else {
                        errors.push(Error::UnresolvedIncludeRef {
                            group_id: unresolved_group.group.id.clone(),
                            include_ref: include.clone(),
                            provenance: unresolved_group.provenance.clone(),
                        });
                    }
                }
            }

            if !attributes_to_import.is_empty() {
                unresolved_group
                    .group
                    .import_attributes_from(attributes_to_import.as_slice());
                unresolved_group
                    .group
                    .update_constraints(any_of_to_import, resolved_includes);
            }
        }

        if errors.is_empty() {
            break;
        }

        // If we still have unresolved `include` but we did not resolve any
        // `include` in the last iteration, we are stuck in an infinite loop.
        // It means that we have an issue with the semantic convention
        // specifications.
        if resolved_include_count == 0 {
            return Err(Error::CompoundError(errors));
        }
    }
    Ok(())
}

fn resolve_inheritance_attrs(
    group_id: &str,
    attrs_group: &[UnresolvedAttribute],
    parent_group_id: &str,
    attrs_parent_group: &[UnresolvedAttribute],
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

    // Inherit the attributes from the parent group.
    for parent_attr in attrs_parent_group.iter() {
        let attr_id = parent_attr.spec.id();
        _ = inherited_attrs.insert(
            attr_id.clone(),
            AttrWithLineage {
                spec: parent_attr.spec.clone(),
                lineage: AttributeLineage::inherit_from(parent_group_id, &parent_attr.spec),
            },
        );
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
                    }
                }
            }
        }
        AttributeSpec::Id { .. } => attr.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::error::Error;
    use std::io::Write;

    use glob::glob;
    use serde::Serialize;
    use tempfile::NamedTempFile;

    use weaver_logger::TestLogger;
    use weaver_resolved_schema::attribute;
    use weaver_resolved_schema::registry::{Constraint, Registry};
    use weaver_semconv::group::GroupType;
    use weaver_semconv::SemConvRegistry;

    use crate::attribute::AttributeCatalog;
    use crate::registry::{check_group_any_of_constraints, resolve_semconv_registry};
    use crate::SchemaResolver;

    /// Test the resolution of semantic convention registries stored in the
    /// data directory. The provided test cases cover the following resolution
    /// scenarios:
    /// - Attribute references.
    /// - Extends references.
    /// - Include constraints.
    /// - Provenance of the attributes (except for the attributes related to
    ///   `include` constraints).
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
        // Iterate over all directories in the data directory and
        // starting with registry-test-*
        for test_entry in glob("data/registry-test-*").expect("Failed to read glob pattern") {
            let path_buf = test_entry.expect("Failed to read test directory");
            let test_dir = path_buf
                .to_str()
                .expect("Failed to convert test directory to string");

            // if !test_dir.ends_with("registry-test-lineage-2") {
            //     // Skip the test for now as it is not yet supported.
            //     continue;
            // }
            println!("Testing `{}`", test_dir);

            let registry_id = "default";
            let sc_specs = SemConvRegistry::try_from_path(
                registry_id,
                &format!("{}/registry/*.yaml", test_dir),
            )
            .expect("Failed to load semconv specs");

            let mut attr_catalog = AttributeCatalog::default();
            let observed_registry =
                resolve_semconv_registry(&mut attr_catalog, "https://127.0.0.1", &sc_specs)
                    .expect("Failed to resolve registry");

            // Check that the resolved attribute catalog matches the expected attribute catalog.
            let observed_attr_catalog = attr_catalog.drain_attributes();

            // Load the expected registry and attribute catalog.
            let expected_attr_catalog: Vec<attribute::Attribute> = serde_json::from_reader(
                std::fs::File::open(format!("{}/expected-attribute-catalog.json", test_dir))
                    .expect("Failed to open expected attribute catalog"),
            )
            .expect("Failed to deserialize expected attribute catalog");

            assert_eq!(
                observed_attr_catalog, expected_attr_catalog,
                "Observed and expected attribute catalogs don't match for `{}`.\nExpected catalog:\n{}\nObserved catalog:\n{}",
                test_dir, to_json(&expected_attr_catalog), to_json(&observed_attr_catalog)
            );

            // let yaml = serde_yaml::to_string(&observed_attr_catalog).unwrap();
            // println!("{}", yaml);
            // println!("Observed registry:\n{}", to_json(&observed_registry));

            // Check that the resolved registry matches the expected registry.
            let expected_registry: Registry = serde_json::from_reader(
                std::fs::File::open(format!("{}/expected-registry.json", test_dir))
                    .expect("Failed to open expected registry"),
            )
            .expect("Failed to deserialize expected registry");

            assert_eq!(
                observed_registry, expected_registry,
                "Expected and observed registry don't match for `{}`.\nObserved registry:\n{}\nExpected registry:\n{}",
                test_dir, to_json(&observed_registry), to_json(&expected_registry)
            );

            // let yaml = serde_yaml::to_string(&observed_registry).unwrap();
            // println!("{}", yaml);
        }
    }

    fn create_registry_from_string(registry_spec: &str) -> Result<Registry, crate::Error> {
        let mut registry_file = NamedTempFile::new().unwrap();
        let _ = registry_file.write_all(registry_spec.as_bytes());

        let registry_id = "default";
        let sc_specs =
            SemConvRegistry::try_from_path(registry_id, registry_file.path().to_str().unwrap())
                .unwrap();

        let mut attr_catalog = AttributeCatalog::default();

        resolve_semconv_registry(&mut attr_catalog, "https://127.0.0.1", &sc_specs)
    }

    #[test]
    #[allow(clippy::print_stdout)]
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
        );

        assert!(result.is_err());

        if let crate::Error::CompoundError(errors) = result.unwrap_err() {
            assert!(errors.len() == 2);
        } else {
            panic!("Expected a CompoundError");
        }
    }

    #[test]
    #[allow(clippy::print_stdout)]
    fn test_registry_error_unresolved_refs() {
        let result = create_registry_from_string(
            "
groups:
    - id: span.one
      type: span
      brief: 'Span one'
      attributes:
        - ref: non.existent.one
          requirement_level: opt_in
        - ref: non.existent.two
          requirement_level: opt_in",
        );

        assert!(result.is_err());

        if let crate::Error::CompoundError(errors) = result.unwrap_err() {
            assert!(errors.len() == 2);
        } else {
            panic!("Expected a CompoundError");
        }
    }

    #[test]
    #[allow(clippy::print_stdout)]
    fn test_registry_error_unresolved_includes() {
        let result = create_registry_from_string(
            "
groups:
    - id: span.one
      type: span
      brief: 'Span one'
      constraints:
        - include: 'non.existent.one'
        - include: 'non.existent.two'
        - include: 'non.existent.three'",
        );

        assert!(result.is_err());

        if let crate::Error::CompoundError(errors) = result.unwrap_err() {
            assert!(errors.len() == 3);
        } else {
            panic!("Expected a CompoundError");
        }
    }

    /// Test the validation of the `any_of` constraints in a group.
    #[test]
    fn test_check_group_any_of_constraints() -> Result<(), crate::Error> {
        // No attribute and no constraint.
        let group_attr_names = HashSet::new();
        let constraints = vec![];
        check_group_any_of_constraints("group", group_attr_names, &constraints)?;

        // Attributes and no constraint.
        let group_attr_names = vec!["attr1".to_owned(), "attr2".to_owned()]
            .into_iter()
            .collect();
        let constraints = vec![];
        check_group_any_of_constraints("group", group_attr_names, &constraints)?;

        // Attributes and multiple constraints (all satisfiable).
        let group_attr_names = vec!["attr1".to_owned(), "attr2".to_owned(), "attr3".to_owned()]
            .into_iter()
            .collect();
        let constraints = vec![
            Constraint {
                any_of: vec!["attr1".to_owned(), "attr2".to_owned()],
                include: None,
            },
            Constraint {
                any_of: vec!["attr3".to_owned()],
                include: None,
            },
            Constraint {
                any_of: vec![],
                include: None,
            },
        ];
        check_group_any_of_constraints("group", group_attr_names, &constraints)?;

        // Attributes and multiple constraints (one unsatisfiable).
        let group_attr_names = vec!["attr1".to_owned(), "attr2".to_owned(), "attr3".to_owned()]
            .into_iter()
            .collect();
        let constraints = vec![
            Constraint {
                any_of: vec!["attr4".to_owned()],
                include: None,
            },
            Constraint {
                any_of: vec![],
                include: None,
            },
        ];
        let result = check_group_any_of_constraints("group", group_attr_names, &constraints);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_api_usage() -> Result<(), Box<dyn Error>> {
        let logger = TestLogger::new();
        let registry_id = "local";
        let registry_dir = "data/registry-test-7-spans/registry/*.yaml";

        // Load a semantic convention registry from a local directory.
        // Note: A method is also available to load a registry from a git
        // repository.
        let mut semconv_registry = SemConvRegistry::try_from_path(registry_id, registry_dir)?;

        // Resolve the semantic convention registry.
        let resolved_schema = SchemaResolver::resolve_semantic_convention_registry(
            &mut semconv_registry,
            logger.clone(),
        )?;

        // Get the resolved registry by its ID.
        let resolved_registry = resolved_schema.registry(registry_id).unwrap();

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
        assert_eq!(span_count, 11, "11 spans in the resolved registry expected");

        Ok(())
    }

    fn to_json<T: Serialize + ?Sized>(value: &T) -> String {
        serde_json::to_string_pretty(value).unwrap()
    }
}
