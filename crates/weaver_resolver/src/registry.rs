// SPDX-License-Identifier: Apache-2.0

//! Functions to resolve a semantic convention registry.

use std::collections::HashMap;

use weaver_resolved_schema::attribute::UnresolvedAttribute;
use weaver_resolved_schema::lineage::{FieldId, FieldLineage, GroupLineage, ResolutionMode};
use weaver_resolved_schema::registry::{Group, Registry, TypedGroup};
use weaver_semconv::attribute::AttributeSpec;
use weaver_semconv::group::ConvTypeSpec;
use weaver_semconv::{GroupSpecWithProvenance, SemConvRegistry};

use crate::attribute::AttributeCatalog;
use crate::constraint::resolve_constraints;
use crate::metrics::resolve_instrument;
use crate::spans::resolve_span_kind;
use crate::stability::resolve_stability;
use crate::{Error, UnresolvedReference};

/// A registry containing unresolved groups.
#[derive(Debug)]
pub struct UnresolvedRegistry {
    /// The semantic convention registry containing resolved groups.
    pub registry: Registry,

    /// List of unresolved groups that belong to the registry.
    /// The resolution process will progressively move the unresolved groups
    /// into the registry field once they are resolved.
    pub groups: Vec<UnresolvedGroup>,
}

/// A group containing unresolved attributes.
#[derive(Debug)]
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
#[allow(dead_code)] // ToDo remove this once this function is called from the CLI.
pub fn resolve_semconv_registry(
    attr_catalog: &mut AttributeCatalog,
    registry_url: &str,
    registry: &SemConvRegistry,
) -> Result<Registry, Error> {
    let mut ureg = unresolved_registry_from_specs(registry_url, registry);
    let mut all_refs_resolved = true;

    all_refs_resolved &= resolve_attribute_references(&mut ureg, attr_catalog);
    all_refs_resolved &= resolve_extends_references(&mut ureg);

    if !all_refs_resolved {
        // Process all unresolved references.
        // An Error::UnresolvedReferences is built and returned.
        let mut unresolved_refs = vec![];
        for group in ureg.groups.iter() {
            if let Some(extends) = group.group.extends.as_ref() {
                unresolved_refs.push(UnresolvedReference::ExtendsRef {
                    group_id: group.group.id.clone(),
                    extends_ref: extends.clone(),
                    provenance: group.provenance.clone(),
                });
            }
            for attr in group.attributes.iter() {
                if let AttributeSpec::Ref { r#ref, .. } = &attr.spec {
                    unresolved_refs.push(UnresolvedReference::AttributeRef {
                        group_id: group.group.id.clone(),
                        attribute_ref: r#ref.clone(),
                        provenance: group.provenance.clone(),
                    });
                }
            }
        }
        if !unresolved_refs.is_empty() {
            return Err(Error::UnresolvedReferences {
                refs: unresolved_refs,
            });
        }
    }

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

    Ok(ureg.registry)
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
#[allow(dead_code)] // ToDo remove this once this function is called from the CLI.
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
            registry_url: registry_url.to_string(),
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
        .collect();

    UnresolvedGroup {
        group: Group {
            id: group.spec.id,
            typed_group: match group.spec.r#type {
                ConvTypeSpec::AttributeGroup => TypedGroup::AttributeGroup {},
                ConvTypeSpec::Span => TypedGroup::Span {
                    span_kind: group.spec.span_kind.as_ref().map(resolve_span_kind),
                    events: group.spec.events,
                },
                ConvTypeSpec::Event => TypedGroup::Event {
                    name: group.spec.name,
                },
                ConvTypeSpec::Metric => TypedGroup::Metric {
                    metric_name: group.spec.metric_name,
                    instrument: group.spec.instrument.as_ref().map(resolve_instrument),
                    unit: group.spec.unit,
                },
                ConvTypeSpec::MetricGroup => TypedGroup::MetricGroup {},
                ConvTypeSpec::Resource => TypedGroup::Resource {},
                ConvTypeSpec::Scope => TypedGroup::Scope {},
            },
            brief: group.spec.brief,
            note: group.spec.note,
            prefix: group.spec.prefix,
            extends: group.spec.extends,
            stability: resolve_stability(&group.spec.stability),
            deprecated: group.spec.deprecated,
            constraints: resolve_constraints(&group.spec.constraints),
            attributes: vec![],
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
) -> bool {
    loop {
        let mut unresolved_attr_count = 0;
        let mut resolved_attr_count = 0;

        // Iterate over all groups and resolve the attributes.
        for unresolved_group in ureg.groups.iter_mut() {
            let mut resolved_attr = vec![];

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
                        unresolved_attr_count += 1;
                        Some(attr)
                    }
                })
                .collect();

            unresolved_group.group.attributes.extend(resolved_attr);
        }

        if unresolved_attr_count == 0 {
            break;
        }
        // If we still have unresolved attributes but we did not resolve any
        // attributes in the last iteration, we are stuck in an infinite loop.
        // It means that we have an issue with the semantic convention
        // specifications.
        if resolved_attr_count == 0 {
            return false;
        }
    }
    true
}

/// Resolves the `extends` references in the given registry.
/// The resolution process is iterative. The process stops when all the
/// `extends` references are resolved or when no `extends` reference could
/// be resolved in an iteration.
///
/// Returns true if all the `extends` references could be resolved.
fn resolve_extends_references(ureg: &mut UnresolvedRegistry) -> bool {
    loop {
        let mut unresolved_extends_count = 0;
        let mut resolved_extends_count = 0;

        // Create a map group_id -> vector of attribute ref for groups
        // that don't have an `extends` clause.
        let mut group_index = HashMap::new();
        for group in ureg.groups.iter() {
            if group.group.extends.is_none() {
                group_index.insert(group.group.id.clone(), group.group.attributes.clone());
            }
        }

        // Iterate over all groups and resolve the `extends` clauses.
        for unresolved_group in ureg.groups.iter_mut() {
            if let Some(extends) = unresolved_group.group.extends.as_ref() {
                if let Some(attr_refs) = group_index.get(extends) {
                    for attr_ref in attr_refs.iter() {
                        unresolved_group.group.attributes.push(*attr_ref);

                        // Update the lineage based on the inherited fields.
                        // Note: the lineage is only updated if a group lineage is provided.
                        if let Some(lineage) = unresolved_group.group.lineage.as_mut() {
                            lineage.add_attribute_field_lineage(
                                *attr_ref,
                                FieldId::GroupAttributes,
                                FieldLineage {
                                    resolution_mode: ResolutionMode::Extends,
                                    group_id: extends.clone(),
                                },
                            );
                        }
                    }
                    unresolved_group.group.extends.take();
                    resolved_extends_count += 1;
                } else {
                    unresolved_extends_count += 1;
                }
            }
        }

        if unresolved_extends_count == 0 {
            break;
        }
        // If we still have unresolved `extends` but we did not resolve any
        // `extends` in the last iteration, we are stuck in an infinite loop.
        // It means that we have an issue with the semantic convention
        // specifications.
        if resolved_extends_count == 0 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use glob::glob;

    use weaver_resolved_schema::attribute;
    use weaver_resolved_schema::registry::Registry;
    use weaver_semconv::SemConvRegistry;

    use crate::attribute::AttributeCatalog;
    use crate::registry::resolve_semconv_registry;

    /// Test the resolution of semantic convention registries stored in the
    /// data directory.
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

            println!("Testing `{}`", test_dir);

            let mut sc_specs = SemConvRegistry::default();
            for sc_entry in
                glob(&format!("{}/registry/*.yaml", test_dir)).expect("Failed to read glob pattern")
            {
                let path_buf = sc_entry.expect("Failed to read semconv file");
                let semconv_file = path_buf
                    .to_str()
                    .expect("Failed to convert semconv file to string");
                let result = sc_specs.load_from_file(semconv_file);
                assert!(
                    result.is_ok(),
                    "Failed to load semconv file `{}, error: {:#?}",
                    semconv_file,
                    result.err().unwrap()
                );
            }

            let mut attr_catalog = AttributeCatalog::default();
            let observed_registry = resolve_semconv_registry(
                &mut attr_catalog,
                "https://semconv-registry.com",
                &sc_specs,
            )
            .expect("Failed to resolve registry");

            // Load the expected registry and attribute catalog.
            let expected_attr_catalog: Vec<attribute::Attribute> = serde_json::from_reader(
                std::fs::File::open(format!("{}/expected-attribute-catalog.json", test_dir))
                    .expect("Failed to open expected attribute catalog"),
            )
            .expect("Failed to deserialize expected attribute catalog");
            let expected_registry: Registry = serde_json::from_reader(
                std::fs::File::open(format!("{}/expected-registry.json", test_dir))
                    .expect("Failed to open expected registry"),
            )
            .expect("Failed to deserialize expected registry");

            // Check that the resolved attribute catalog matches the expected attribute catalog.
            let observed_attr_catalog = attr_catalog.drain_attributes();
            let observed_attr_catalog_json = serde_json::to_string_pretty(&observed_attr_catalog)
                .expect("Failed to serialize observed attribute catalog");

            assert_eq!(
                observed_attr_catalog, expected_attr_catalog,
                "Attribute catalog does not match for `{}`.\nObserved catalog:\n{}",
                test_dir, observed_attr_catalog_json
            );

            let yaml = serde_yaml::to_string(&observed_attr_catalog).unwrap();
            println!("{}", yaml);

            // Check that the resolved registry matches the expected registry.
            let observed_registry_json = serde_json::to_string_pretty(&observed_registry)
                .expect("Failed to serialize observed registry");

            assert_eq!(
                observed_registry, expected_registry,
                "Registry does not match for `{}`.\nObserved registry:\n{}",
                test_dir, observed_registry_json
            );

            let yaml = serde_yaml::to_string(&observed_registry).unwrap();
            println!("{}", yaml);
        }
    }
}

// ToDo Remove #[allow(dead_code)] once the corresponding functions are called from the CLI.
