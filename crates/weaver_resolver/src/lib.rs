// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

pub(crate) mod engine;
mod error;
pub(crate) mod graph;
pub(crate) mod v1;
pub(crate) mod v2;

#[cfg(test)]
mod refinement_association_tests;
#[cfg(test)]
mod unref_tests;
#[cfg(test)]
mod upgrade_tests;

pub use crate::engine::{
    DefaultSchemaVisitor, NullSchemaResolver, ResolutionTarget, ResolvedSchemaCache,
    SchemaLoadingVisitor, SchemaResolver, WeaverResolvedSchema, WeaverResolver,
    WeaverResolverConfig,
};
pub use crate::error::Error;
#[cfg(test)]
pub(crate) use crate::graph::chosen_versions::SchemaLookupContext;
pub use crate::graph::loader::{LoadedSemconvRegistry, SourceFormat};

#[cfg(test)]
pub(crate) use crate::v1::dependency::ResolvedDependency;
#[cfg(test)]
pub(crate) use crate::v1::DisallowV1EngineScope;
#[cfg(test)]
pub(crate) use crate::v2::DisallowV1AdapterScope;
#[cfg(test)]
pub(crate) use std::sync::Arc;
#[cfg(test)]
pub(crate) use weaver_common::result::WResult;
#[cfg(test)]
pub(crate) use weaver_resolved_schema::v1::ResolvedTelemetrySchema;
#[cfg(test)]
pub(crate) use weaver_resolved_schema::v2::ResolvedTelemetrySchema as V2Schema;
#[cfg(test)]
pub(crate) use weaver_semconv::registry_repo::RegistryRepo;
#[cfg(test)]
pub(crate) use weaver_semconv::schema_url::SchemaUrl;

// -----------------------------------------------------------------------------
// Test Suites
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use lru::LruCache;
    use std::collections::{BTreeMap, HashMap, HashSet};
    use std::num::NonZeroUsize;
    use weaver_common::vdir::VirtualDirectoryPath;
    use weaver_semconv::v1::attribute::{BasicRequirementLevelSpec, RequirementLevel};
    use weaver_semconv::v1::group::GroupType;
    #[test]
    fn test_weaver_resolver_caching() {
        let config = WeaverResolverConfig::default();
        let mut resolver = WeaverResolver::new(config);

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/custom_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("Failed to create RegistryRepo");

        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let url = SchemaUrl::try_from(resolved.schema_url.as_str()).expect("Valid schema url");
        let cached1 = match resolver.resolve_schema(&url) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Failed to get from cache: {e}"),
        };
        let cached2 = match resolver.resolve_schema(&url) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Failed to get from cache: {e}"),
        };

        assert!(Arc::ptr_eq(&cached1, &cached2));
    }

    #[test]
    fn test_multi_registry() -> Result<(), weaver_semconv::Error> {
        fn check_semconv_load_and_resolve(registry_repo: RegistryRepo, include_unreferenced: bool) {
            let config = WeaverResolverConfig {
                include_unreferenced,
                ..Default::default()
            };
            let mut resolver = WeaverResolver::new(config);
            let resolved =
                match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
                    WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
                    WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
                };

            let resolved_registry = &resolved;

            if include_unreferenced {
                let group = resolved_registry.group("otel.unused");
                assert!(group.is_some());

                let group = resolved_registry.group("metric.example.counter");
                assert!(group.is_some());
                let group = resolved_registry.group("entity.gcp.apphub.application");
                assert!(group.is_some());
                let group = resolved_registry.group("entity.gcp.apphub.service");
                assert!(group.is_some());
                let group = resolved_registry.group("event.session.start");
                assert!(group.is_some());
                let group = resolved_registry.group("event.session.end");
                assert!(group.is_some());
                let group = resolved_registry.group("custom.group");
                assert!(group.is_some());
                let group = resolved_registry.group("custom.span");
                assert!(group.is_some());
            } else {
                let group = resolved_registry.group("otel.unused");
                assert!(group.is_none());
                let group = resolved_registry.group("event.session.end");
                assert!(group.is_none());

                let group = resolved_registry.group("metric.example.counter");
                assert!(group.is_some());
                let group = resolved_registry.group("entity.gcp.apphub.application");
                assert!(group.is_some());
                let group = resolved_registry.group("entity.gcp.apphub.service");
                assert!(group.is_some());
                let group = resolved_registry.group("event.session.start");
                assert!(group.is_some());
                let group = resolved_registry.group("custom.group");
                assert!(group.is_some());
                let group = resolved_registry.group("custom.span");
                assert!(group.is_some());
            }

            let metrics = resolved_registry.groups(GroupType::Metric);
            let metric = metrics
                .get("metric.auction.bid.count")
                .expect("Metric not found");
            let attributes = &metric.attributes;
            assert_eq!(attributes.len(), 3);
            let mut attr_names = HashSet::new();
            for attr_ref in attributes {
                let attr = resolved
                    .catalog
                    .attribute(attr_ref)
                    .expect("Failed to resolve attribute");
                _ = attr_names.insert(attr.name.clone());
                match attr.name.as_str() {
                    "auction.name" => {}
                    "auction.id" => {}
                    "error.type" => {
                        assert_eq!(
                            attr.requirement_level,
                            RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
                        );
                        assert_eq!(attr.brief, "The error type.".to_owned());
                    }
                    _ => {
                        panic!("Unexpected attribute name: {}", attr.name);
                    }
                }
            }
            assert_eq!(metric.attributes.len(), 3);
            assert!(attr_names.contains("auction.name"));
            assert!(attr_names.contains("auction.id"));
            assert!(attr_names.contains("error.type"));
        }

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/custom_registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        check_semconv_load_and_resolve(registry_repo.clone(), false);
        check_semconv_load_and_resolve(registry_repo.clone(), true);
        Ok(())
    }

    #[test]
    fn test_three_registry_chain_works() -> Result<(), weaver_semconv::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/multi-registry/app_registry/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let config = WeaverResolverConfig {
            follow_symlinks: true,
            ..Default::default()
        };
        let mut resolver = WeaverResolver::new(config);

        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let resolved_registry = &resolved;

        let all_groups: Vec<String> = [
            GroupType::AttributeGroup,
            GroupType::Metric,
            GroupType::Event,
            GroupType::Span,
        ]
        .iter()
        .flat_map(|group_type| {
            resolved_registry
                .groups(group_type.clone())
                .keys()
                .map(|k| (*k).to_owned())
                .collect::<Vec<_>>()
        })
        .collect();

        assert_eq!(
            all_groups.len(),
            2,
            "Expected 2 groups (app.example and metric.example.counter), but found {}: {:?}",
            all_groups.len(),
            all_groups
        );
        assert!(
            all_groups.contains(&"app.example".to_owned()),
            "Missing app.example group, found: {all_groups:?}"
        );
        assert!(
            all_groups.contains(&"metric.example.counter".to_owned()),
            "Missing metric.example.counter group, found: {all_groups:?}"
        );

        let app_group = resolved_registry
            .group("app.example")
            .expect("app.example group should exist");

        let mut attr_names = HashSet::new();
        for attr_ref in &app_group.attributes {
            let attr = resolved
                .catalog
                .attribute(attr_ref)
                .expect("Failed to resolve attribute");
            let _ = attr_names.insert(attr.name.clone());
        }

        assert!(
            attr_names.contains("app.name"),
            "Missing app.name attribute"
        );
        assert!(
            attr_names.contains("error.type"),
            "Missing error.type attribute"
        );
        assert!(
            attr_names.contains("auction.name"),
            "Missing auction.name attribute"
        );
        assert_eq!(attr_names.len(), 3,
            "Expected exactly 3 attributes (app.name, error.type, auction.name), got: {attr_names:?}");
        Ok(())
    }

    #[test]
    fn test_v2_dependency_resolution() -> Result<(), weaver_semconv::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/consumer_registry/registry".to_owned(),
        };

        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());

        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let resolved_registry = &resolved;
        let metrics = resolved_registry.groups(GroupType::Metric);
        let metric = metrics
            .get("metric.consumer.request.count")
            .expect("metric.consumer.request.count not found");

        assert_eq!(metric.attributes.len(), 2);

        let mut attr_names = HashSet::new();
        for attr_ref in &metric.attributes {
            let attr = resolved
                .catalog
                .attribute(attr_ref)
                .expect("Failed to resolve attribute ref");
            _ = attr_names.insert(attr.name.clone());
            match attr.name.as_str() {
                "server.address" => {
                    assert_eq!(
                        attr.requirement_level,
                        RequirementLevel::Basic(BasicRequirementLevelSpec::Required)
                    );
                    assert_eq!(attr.brief, "Server address.");
                    assert_eq!(
                        attr.r#type,
                        weaver_semconv::v1::attribute::AttributeType::PrimitiveOrArray(
                            weaver_semconv::v1::attribute::PrimitiveOrArrayTypeSpec::String
                        )
                    );
                }
                "server.port" => {
                    assert_eq!(attr.brief, "The server port used by the consumer.");
                    assert_eq!(
                        attr.r#type,
                        weaver_semconv::v1::attribute::AttributeType::PrimitiveOrArray(
                            weaver_semconv::v1::attribute::PrimitiveOrArrayTypeSpec::Int
                        )
                    );
                }
                _ => panic!("Unexpected attribute: {}", attr.name),
            }
        }

        assert!(attr_names.contains("server.address"));
        assert!(attr_names.contains("server.port"));
        Ok(())
    }

    #[test]
    fn test_v2_entity_refinement_preserves_inherited_v1_identity(
    ) -> Result<(), weaver_semconv::Error> {
        use weaver_resolved_schema::v2::catalog::AttributeCatalog;

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/entity-identity-inheritance/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let resolved = resolver
            .load_and_resolve_schema(registry_repo, DefaultSchemaVisitor)
            .ignore(is_unstable_format_warning)
            .into_result_failing_non_fatal()
            .expect("A refinement must preserve identity inherited from a v1 attribute definition")
            .into_v1()
            .unwrap();
        let schema: weaver_resolved_schema::v2::ResolvedTelemetrySchema =
            resolved.try_into().expect("Failed to convert to v2 schema");
        let base = schema
            .registry
            .entities
            .iter()
            .find(|entity| entity.r#type.to_string() == "host")
            .expect("Missing base entity");
        let refinement = schema
            .refinements
            .entities
            .iter()
            .find(|refinement| refinement.id.to_string() == "refined.host")
            .expect("Missing entity refinement");
        for entity in [base, &refinement.entity] {
            assert_eq!(entity.identity.len(), 1);
            assert_eq!(
                schema.attribute_catalog.attribute_key(&entity.identity[0]),
                Some("host.id")
            );
            assert!(entity.description.is_empty());
        }
        Ok(())
    }

    /// A v1 base entity can take its identity from a dependency attribute
    /// whose role is set on the definition. Refining it keeps that identity.
    #[test]
    fn test_v2_entity_refinement_preserves_identity_from_dependency_attribute(
    ) -> Result<(), weaver_semconv::Error> {
        use weaver_resolved_schema::v2::catalog::AttributeCatalog;

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/entity-identity-dependency-role/consumer/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let resolved = resolver
            .load_and_resolve_schema(registry_repo, DefaultSchemaVisitor)
            .ignore(is_unstable_format_warning)
            .into_result_failing_non_fatal()
            .expect("A refinement must preserve identity from a dependency attribute")
            .into_v1()
            .unwrap();
        let schema: weaver_resolved_schema::v2::ResolvedTelemetrySchema =
            resolved.try_into().expect("Failed to convert to v2 schema");
        let refinement = schema
            .refinements
            .entities
            .iter()
            .find(|refinement| refinement.id.to_string() == "refined.local")
            .expect("Missing entity refinement");
        assert_eq!(refinement.entity.identity.len(), 1);
        assert_eq!(
            schema
                .attribute_catalog
                .attribute_key(&refinement.entity.identity[0]),
            Some("dep.id")
        );
        Ok(())
    }

    /// A local definition without a role shadows a dependency's identifying
    /// definition of the same attribute, so a refinement can't promote it.
    #[test]
    fn test_v2_entity_refinement_cannot_promote_local_shadowing_attribute(
    ) -> Result<(), weaver_semconv::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/entity-identity-dependency-role/shadowing".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let error = resolver
            .load_and_resolve_schema(registry_repo, DefaultSchemaVisitor)
            .ignore(is_unstable_format_warning)
            .into_result_failing_non_fatal()
            .expect_err("Promoting a descriptive attribute to identity must fail");
        let message = format!("{error:?}");
        assert!(
            message.contains("EntityRefinementChangedIdentity")
                && message.contains("attribute_id: \"dep.id\""),
            "got: {message}"
        );
        Ok(())
    }

    /// A refinement can't `unref` an identity attribute of a v1 base entity,
    /// even when the role is set only on the attribute definition. The base
    /// has two identity attributes, so v2 conversion alone wouldn't notice.
    #[test]
    fn test_v2_entity_refinement_cannot_unref_inherited_v1_identity(
    ) -> Result<(), weaver_semconv::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/entity-identity-inheritance-unref".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let error = resolver
            .load_and_resolve_schema(registry_repo, DefaultSchemaVisitor)
            .ignore(is_unstable_format_warning)
            .into_result_failing_non_fatal()
            .expect_err("Removing an inherited identity attribute must fail");
        let message = format!("{error:?}");
        assert!(
            message.contains("InvalidAttributeUnref")
                && message.contains("host.name")
                && message.contains("refinements must preserve identity"),
            "got: {message}"
        );
        Ok(())
    }

    /// An attribute a dependency inherited rather than defined reaches this
    /// registry only through the signals that carry it. Refining such a signal
    /// must not turn the attribute into something a bare `ref` can resolve
    /// against - that would resolve to the refinement's own copy.
    #[test]
    fn test_inherited_attribute_is_not_a_definition() -> Result<(), weaver_semconv::Error> {
        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let registry_path = VirtualDirectoryPath::LocalFolder {
                path: "data/inherited-ref-test/user".to_owned(),
            };
            let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
            let mut resolver = WeaverResolver::new(WeaverResolverConfig {
                target,
                ..Default::default()
            });

            match resolver
                .load_and_resolve_schema(registry_repo, DefaultSchemaVisitor)
                .into_result_failing_non_fatal()
            {
                Ok(_) => panic!(
                    "expected `base.attr` to be unresolvable in `user.metric` on target {target:?}"
                ),
                Err(e) => {
                    let msg = e.to_string();
                    assert!(
                        msg.contains("metric.user.metric") && msg.contains("base.attr"),
                        "Expected an unresolved attribute reference on target {target:?}, got: {msg}"
                    );
                }
            }
        }
        Ok(())
    }

    /// Same as above, but resolving the dependency from source rather than from
    /// a published artifact. Both forms must refuse the reference - otherwise a
    /// registry resolves differently depending on how it was delivered.
    #[test]
    fn test_inherited_attribute_is_not_a_definition_from_source(
    ) -> Result<(), weaver_semconv::Error> {
        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let registry_path = VirtualDirectoryPath::LocalFolder {
                path: "data/inherited-ref-test/refiner_user".to_owned(),
            };
            let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
            let mut resolver = WeaverResolver::new(WeaverResolverConfig {
                target,
                ..Default::default()
            });

            match resolver
                .load_and_resolve_schema(registry_repo, DefaultSchemaVisitor)
                .into_result_failing_non_fatal()
            {
                Ok(_) => panic!("expected `base.attr` to be unresolvable in `refiner.user.metric` on target {target:?}"),
                Err(e) => {
                    let msg = e.to_string();
                    assert!(
                        msg.contains("metric.refiner.user.metric") && msg.contains("base.attr"),
                        "Expected an unresolved attribute reference on target {target:?}, got: {msg}"
                    );
                }
            }
        }
        Ok(())
    }

    /// Can't demote an identity attribute of a base entity.
    #[test]
    fn test_v2_dependency_entity_identity_demotion_rejected() -> Result<(), weaver_semconv::Error> {
        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let registry_path = VirtualDirectoryPath::LocalFolder {
                path: "data/registry-test-v2-dep/entity_demote_registry/registry".to_owned(),
            };
            let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
            let mut resolver = WeaverResolver::new(WeaverResolverConfig {
                target,
                ..Default::default()
            });

            match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
                WResult::Ok(_) | WResult::OkWithNFEs(_, _) => {
                    panic!("Expected resolution to fail with an identity demotion error on target {target:?}")
                }
                WResult::FatalErr(e) => {
                    let msg = e.to_string();
                    assert!(
                        msg.contains("Entity refinement `demoted.host` changes the identity of")
                            && msg.contains("referencing `host.id` under `Descriptive`"),
                        "Expected identity demotion error on target {target:?}, got: {msg}"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_v2_three_layer_dependency_resolution() -> Result<(), weaver_semconv::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/app_registry/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());

        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let resolved_registry = &resolved;
        let metrics = resolved_registry.groups(GroupType::Metric);
        let metric = metrics
            .get("metric.app.request.count")
            .expect("metric.app.request.count not found");
        assert_eq!(metric.attributes.len(), 2);
        for attr_ref in &metric.attributes {
            let attr = resolved
                .catalog
                .attribute(attr_ref)
                .expect("Failed to resolve attribute ref");
            match attr.name.as_str() {
                "server.address" => assert_eq!(attr.brief, "Server address."),
                "server.port" => assert_eq!(attr.brief, "Server port."),
                _ => panic!("Unexpected attribute: {}", attr.name),
            }
        }
        Ok(())
    }

    #[test]
    fn test_schema_url_overrides() -> Result<(), Error> {
        let schema_url = SchemaUrl::try_from("https://app.example.com/schemas/1.0.0").unwrap();
        let override_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/app_registry/registry".to_owned(),
        };

        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let mut config = WeaverResolverConfig {
                target,
                ..Default::default()
            };
            _ = config
                .schema_url_overrides
                .insert(schema_url.clone(), override_path.clone());

            let mut resolver = WeaverResolver::new(config);

            let resolved = match resolver.resolve_schema(&schema_url) {
                WResult::Ok(r) => r,
                WResult::OkWithNFEs(r, nfes) => {
                    let real_nfes: Vec<_> = nfes
                        .into_iter()
                        .filter(|e| {
                            !matches!(
                                e,
                                Error::V1DependencyUsesV1Path { .. }
                                    | Error::FailToResolveDefinition(
                                        weaver_semconv::Error::UnstableFileFormat { .. }
                                            | weaver_semconv::Error::MissingManifestFileFormat { .. },
                                    )
                            )
                        })
                        .collect();
                    assert!(
                        real_nfes.is_empty(),
                        "spurious warnings on target {target:?}: {real_nfes:?}"
                    );
                    r
                }
                WResult::FatalErr(e) => {
                    panic!("Failed to resolve overridden schema on target {target:?}: {e}")
                }
            };

            assert_eq!(
                resolved.schema_url_str(),
                "https://app.example.com/schemas/1.0.0"
            );
        }
        Ok(())
    }

    #[test]
    fn test_schema_url_overrides_mismatch() -> Result<(), Error> {
        let schema_url = SchemaUrl::try_from("https://mismatch.example.com/schemas/1.0.0").unwrap();
        let override_path = VirtualDirectoryPath::LocalFolder {
            path: "data/registry-test-v2-dep/app_registry/registry".to_owned(),
        };

        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let mut config = WeaverResolverConfig {
                target,
                ..Default::default()
            };
            _ = config
                .schema_url_overrides
                .insert(schema_url.clone(), override_path.clone());

            let mut resolver = WeaverResolver::new(config);

            match resolver.resolve_schema(&schema_url) {
                WResult::FatalErr(Error::MismatchSchemaUrl { expected, actual }) => {
                    assert_eq!(expected, "https://mismatch.example.com/schemas/1.0.0");
                    assert_eq!(actual, "https://app.example.com/schemas/1.0.0");
                }
                WResult::Ok(_) => {
                    panic!("Expected FatalErr(MismatchSchemaUrl) on target {target:?}, got Ok")
                }
                WResult::OkWithNFEs(_, _) => {
                    panic!(
                        "Expected FatalErr(MismatchSchemaUrl) on target {target:?}, got OkWithNFEs"
                    )
                }
                WResult::FatalErr(e) => panic!(
                    "Expected FatalErr(MismatchSchemaUrl) on target {target:?}, got FatalErr({:?})",
                    e
                ),
            }
        }
        Ok(())
    }

    fn resolve_at_with_config(
        path: &str,
        config: WeaverResolverConfig,
    ) -> WResult<WeaverResolvedSchema, Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: path.to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("Failed to create registry repo");
        let mut resolver = WeaverResolver::new(config);
        resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor)
    }

    fn resolve_inline_with_parent_and_config(
        consumer_yaml: &str,
        parent_path: &str,
        config: WeaverResolverConfig,
    ) -> WResult<WeaverResolvedSchema, Error> {
        let parent_vpath = VirtualDirectoryPath::LocalFolder {
            path: parent_path.to_owned(),
        };
        let parent_repo = RegistryRepo::try_new(None, &parent_vpath, &mut vec![])
            .expect("Failed to create parent registry repo");
        let mut resolver = WeaverResolver::new(config);
        let parent_loaded = match resolver.load_repository(parent_repo) {
            WResult::Ok(l) | WResult::OkWithNFEs(l, _) => l,
            WResult::FatalErr(e) => panic!("Failed to load parent: {e}"),
        };

        let consumer = LoadedSemconvRegistry::create_from_string(consumer_yaml)
            .expect("Failed to load consumer yaml");
        let with_dep = match consumer {
            LoadedSemconvRegistry::Unresolved { repo, specs, .. } => {
                LoadedSemconvRegistry::Unresolved {
                    repo,
                    specs,
                    dependencies: vec![parent_loaded],
                }
            }
            _ => panic!("Expected unresolved consumer registry"),
        };

        resolver.resolve_loaded(with_dep).map(Arc::unwrap_or_clone)
    }

    fn assert_exclusion_errors(
        result: WResult<WeaverResolvedSchema, Error>,
        expected: &[(&str, &str)],
    ) {
        let err = match result {
            WResult::Ok(_) | WResult::OkWithNFEs(_, _) => {
                panic!("Expected an exclusion error, got Ok");
            }
            WResult::FatalErr(e) => e,
        };
        let mut errors = vec![];
        collect_errors(&err, &mut errors);
        for (expected_id, expected_used_in) in expected {
            let found = errors.iter().any(|e| {
                matches!(
                    e,
                    Error::ExcludedFromDependencyResolution { id, used_in, .. }
                        if id == expected_id && used_in == expected_used_in
                )
            });
            assert!(
                found,
                "expected ExcludedFromDependencyResolution(id={expected_id}, used_in={expected_used_in}); got {errors:#?}"
            );
        }
    }

    fn collect_errors<'a>(err: &'a Error, out: &mut Vec<&'a Error>) {
        match err {
            Error::CompoundError(inner) => {
                for e in inner {
                    collect_errors(e, out);
                }
            }
            other => out.push(other),
        }
    }

    const PUBLISHED_V2_PATH: &str = "data/registry-test-dep-exclusion/published_v2";

    #[test]
    fn test_dep_exclusion_v2_fails() {
        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let config = WeaverResolverConfig {
                target,
                ..Default::default()
            };
            // Resolver is fail-fast across stages (extends, attr refs, imports),
            // so each leak path is exercised in its own minimal consumer spec.
            // Inline YAML keeps the test bodies adjacent to their assertions.
            let ref_yaml = r#"
file_format: definition/2
metrics:
  - name: consumer.requests
    brief: References an excluded parent attribute.
    instrument: counter
    unit: "{request}"
    stability: stable
    requirement_level: recommended
    attributes:
      - ref: parent.excluded
        requirement_level: required
"#;
            let ref_res =
                resolve_inline_with_parent_and_config(ref_yaml, PUBLISHED_V2_PATH, config.clone());
            match target {
                ResolutionTarget::V1 => {
                    assert_exclusion_errors(
                        ref_res,
                        &[("parent.excluded", "metric.consumer.requests")],
                    );
                }
                ResolutionTarget::V2 => {
                    let mut errors = Vec::new();
                    match &ref_res {
                        WResult::FatalErr(e) => collect_errors(e, &mut errors),
                        WResult::OkWithNFEs(_, nfes) => {
                            for e in nfes {
                                collect_errors(e, &mut errors);
                            }
                        }
                        WResult::Ok(_) => panic!("expected error for excluded ref"),
                    }
                    assert!(
                        errors.iter().any(|e| matches!(
                            e,
                            Error::UnresolvedAttributeRef {
                                group_id,
                                attribute_ref,
                                ..
                            } if group_id == "metric.consumer.requests" && attribute_ref == "parent.excluded"
                        )),
                        "expected UnresolvedAttributeRef for parent.excluded, got {errors:#?}"
                    );
                }
            }

            let extends_yaml = r#"
groups:
  - id: consumer.requests
    type: metric
    metric_name: consumer.requests
    instrument: counter
    unit: "1"
    stability: stable
    brief: Extends an excluded parent metric.
    extends: parent.excluded.metric
"#;
            let v2_refinement_yaml = r#"
file_format: definition/2
metric_refinements:
  - id: consumer.requests
    ref: parent.excluded.metric
    brief: Refines an excluded parent metric.
"#;
            match target {
                ResolutionTarget::V1 => {
                    assert_exclusion_errors(
                        resolve_inline_with_parent_and_config(
                            extends_yaml,
                            PUBLISHED_V2_PATH,
                            config.clone(),
                        ),
                        &[("parent.excluded.metric", "consumer.requests")],
                    );
                }
                ResolutionTarget::V2 => {
                    let v2_ref_res = resolve_inline_with_parent_and_config(
                        v2_refinement_yaml,
                        PUBLISHED_V2_PATH,
                        config.clone(),
                    );
                    let mut errors = Vec::new();
                    match &v2_ref_res {
                        WResult::FatalErr(e) => collect_errors(e, &mut errors),
                        WResult::OkWithNFEs(_, nfes) => {
                            for e in nfes {
                                collect_errors(e, &mut errors);
                            }
                        }
                        WResult::Ok(_) => panic!("expected error for excluded refinement parent"),
                    }
                    assert!(
                        errors.iter().any(|e| matches!(
                            e,
                            Error::UnresolvedRefinementParent {
                                refinement_id,
                                r#ref,
                                ..
                            } if refinement_id == "consumer.requests" && r#ref == "parent.excluded.metric"
                        )),
                        "expected UnresolvedRefinementParent for parent.excluded.metric, got {errors:#?}"
                    );
                }
            }

            let imports_yaml = r#"
file_format: definition/2
imports:
  metrics:
    - parent.excluded.metric
"#;
            let imports_res =
                resolve_inline_with_parent_and_config(imports_yaml, PUBLISHED_V2_PATH, config);
            match target {
                ResolutionTarget::V1 => {
                    assert_exclusion_errors(imports_res, &[("parent.excluded.metric", "imports")]);
                }
                ResolutionTarget::V2 => {
                    let (resolved, nfes) = match imports_res {
                        WResult::OkWithNFEs(r, nfes) => (r, nfes),
                        WResult::FatalErr(e) => {
                            panic!("expected OkWithNFEs for unmatched import warning, got FatalErr: {e:?}")
                        }
                        WResult::Ok(_) => {
                            panic!("expected OkWithNFEs for unmatched import warning, got Ok without NFEs")
                        }
                    };
                    assert!(
                        nfes.iter().any(|e| matches!(
                            e,
                            Error::UnmatchedImport { pattern, signal }
                                if pattern == "parent.excluded.metric" && signal == "metrics"
                        )),
                        "expected UnmatchedImport warning, got {nfes:#?}"
                    );
                    assert!(resolved.as_v2().unwrap().registry.metrics.is_empty());
                }
            }
        }
    }

    #[test]
    fn test_dep_exclusion_v2_excluded_user_still_fails() {
        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let config = WeaverResolverConfig {
                target,
                ..Default::default()
            };
            // Cross-registry references to an excluded item ALWAYS fail, even when
            // the consumer's own using item is marked excluded. The boundary rule
            // is absolute: excluded items in a dependency are invisible during
            // resolution. (The within-registry "both excluded → ok" relaxation is
            // covered by `test_within_registry_both_excluded`.)
            let consumer = r#"
file_format: definition/2
metrics:
  - name: consumer.also.excluded
    brief: Consumer metric that itself is excluded.
    instrument: counter
    unit: "{request}"
    stability: stable
    requirement_level: recommended
    annotations:
      dependency_resolution:
        exclude: true
    attributes:
      - ref: parent.excluded
        requirement_level: required
"#;
            let res = resolve_inline_with_parent_and_config(consumer, PUBLISHED_V2_PATH, config);
            match target {
                ResolutionTarget::V1 => {
                    assert_exclusion_errors(
                        res,
                        &[("parent.excluded", "metric.consumer.also.excluded")],
                    );
                }
                ResolutionTarget::V2 => {
                    let mut errors = Vec::new();
                    match &res {
                        WResult::FatalErr(e) => collect_errors(e, &mut errors),
                        WResult::OkWithNFEs(_, nfes) => {
                            for e in nfes {
                                collect_errors(e, &mut errors);
                            }
                        }
                        WResult::Ok(_) => panic!("expected error for excluded ref"),
                    }
                    assert!(
                        errors.iter().any(|e| matches!(
                            e,
                            Error::UnresolvedAttributeRef {
                                group_id,
                                attribute_ref,
                                ..
                            } if group_id == "metric.consumer.also.excluded" && attribute_ref == "parent.excluded"
                        )),
                        "expected UnresolvedAttributeRef for parent.excluded, got {errors:#?}"
                    );
                }
            }
        }
    }

    fn create_registry_from_string(
        registry_spec: &str,
    ) -> WResult<weaver_resolved_schema::v1::registry::Registry, Error> {
        let loaded = LoadedSemconvRegistry::create_from_string(registry_spec)
            .expect("Failed to load semconv spec");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        match resolver.resolve_loaded(loaded) {
            WResult::Ok(arc) => Arc::unwrap_or_clone(arc)
                .into_v1()
                .map(|s| WResult::Ok(s.registry))
                .unwrap_or_else(WResult::FatalErr),
            WResult::OkWithNFEs(arc, nfes) => match Arc::unwrap_or_clone(arc).into_v1() {
                Ok(v1) => WResult::OkWithNFEs(v1.registry, nfes),
                Err(e) => WResult::FatalErr(e),
            },
            WResult::FatalErr(e) => WResult::FatalErr(e),
        }
    }

    #[test]
    fn test_within_registry_leak_v1_ref() {
        // Same registry: an excluded attribute is defined inside an excluded
        // host group (so its definition is fine), but a non-excluded span
        // refs it. The ref is the leak path.
        let result = create_registry_from_string(
            "
groups:
    - id: attrs.private
      type: attribute_group
      brief: Private
      annotations:
        dependency_resolution:
          exclude: true
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true
    - id: span.public
      type: span
      span_kind: internal
      stability: stable
      brief: A public span that leaks an excluded attribute.
      attributes:
        - ref: secret.value
          requirement_level: required",
        );

        match result.into_result_failing_non_fatal() {
            Ok(_) => panic!("expected an exclusion error"),
            Err(Error::CompoundError(errors)) => {
                assert!(
                    errors.iter().any(|e| matches!(
                        e,
                        Error::ExcludedFromDependencyResolution { id, used_in, .. }
                            if id == "secret.value" && used_in == "span.public"
                    )),
                    "expected exclusion error on span.public, got {errors:#?}"
                );
            }
            Err(e) => panic!("expected CompoundError, got {e:?}"),
        }
    }

    #[test]
    fn test_within_registry_leak_inline_id() {
        // A non-excluded group defines an attribute with `id:` that is marked
        // excluded. Defining it inline means the group inlines that attribute
        // into its resolved form — a leak.
        let result = create_registry_from_string(
            "
groups:
    - id: span.public
      type: span
      span_kind: internal
      stability: stable
      brief: Public span with an inlined excluded attribute.
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true",
        );

        match result.into_result_failing_non_fatal() {
            Ok(_) => panic!("expected an exclusion error"),
            Err(Error::CompoundError(errors)) => {
                assert!(
                    errors.iter().any(|e| matches!(
                        e,
                        Error::ExcludedFromDependencyResolution { id, used_in, .. }
                            if id == "secret.value" && used_in == "span.public"
                    )),
                    "expected exclusion error from inline-id, got {errors:#?}"
                );
            }
            Err(e) => panic!("expected CompoundError, got {e:?}"),
        }
    }

    #[test]
    fn test_within_registry_both_excluded() {
        // When the using group is also excluded, leak validation is skipped.
        let result = create_registry_from_string(
            "
groups:
    - id: attrs.private
      type: attribute_group
      brief: Private
      annotations:
        dependency_resolution:
          exclude: true
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true
    - id: span.also.excluded
      type: span
      span_kind: internal
      stability: stable
      brief: Also excluded.
      annotations:
        dependency_resolution:
          exclude: true
      attributes:
        - ref: secret.value
          requirement_level: required",
        );

        match result.into_result_failing_non_fatal() {
            Ok(_) => {}
            Err(e) => panic!("expected success when both are excluded; got {e:?}"),
        }
    }

    #[test]
    fn test_dep_exclusion_migration_redefine() {
        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let config = WeaverResolverConfig {
                target,
                ..Default::default()
            };
            // Parent registry has an attribute, a metric, and a span — all
            // deprecated and excluded from dependency resolution. The consumer
            // depends on the parent and redefines exactly the same items.
            // Resolution must succeed: the parent items are hidden from
            // dependents, so the consumer's redefinitions take effect.
            let result = resolve_at_with_config(
                "data/registry-test-dep-exclusion/migration_consumer/registry",
                config,
            );
            let resolved = match result {
                WResult::Ok(s) | WResult::OkWithNFEs(s, _) => s,
                WResult::FatalErr(e) => panic!("expected success; got {e:?}"),
            };

            match resolved {
                WeaverResolvedSchema::V1(v1) => {
                    let attrs: Vec<&str> =
                        v1.catalog.attributes().map(|a| a.name.as_str()).collect();
                    assert!(
                        attrs.contains(&"moved.attr"),
                        "expected moved.attr in catalog, got {attrs:?}"
                    );

                    let metric = v1
                        .groups(GroupType::Metric)
                        .get("metric.moved.metric")
                        .cloned()
                        .expect("metric.moved.metric should be present");
                    assert_eq!(
                        metric.deprecated, None,
                        "consumer metric must not inherit parent deprecation"
                    );

                    let span = v1
                        .groups(GroupType::Span)
                        .get("span.moved.span")
                        .cloned()
                        .expect("span.moved.span should be present");
                    assert_eq!(
                        span.deprecated, None,
                        "consumer span must not inherit parent deprecation"
                    );

                    // Greenfield metric (no parent equivalent, no refs) resolves cleanly
                    // alongside the redefined items.
                    assert!(
                        v1.groups(GroupType::Metric)
                            .contains_key("metric.greenfield.requests"),
                        "greenfield metric should resolve"
                    );
                }
                WeaverResolvedSchema::V2(v2) => {
                    assert!(
                        v2.attribute_catalog.iter().any(|a| a.key == "moved.attr"),
                        "expected moved.attr in catalog"
                    );

                    let metric = v2
                        .registry
                        .metrics
                        .iter()
                        .find(|m| &*m.name == "moved.metric")
                        .expect("metric.moved.metric should be present");
                    assert_eq!(
                        metric.common.deprecated, None,
                        "consumer metric must not inherit parent deprecation"
                    );

                    let span = v2
                        .registry
                        .spans
                        .iter()
                        .find(|s| &*s.r#type == "moved.span")
                        .expect("span.moved.span should be present");
                    assert_eq!(
                        span.common.deprecated, None,
                        "consumer span must not inherit parent deprecation"
                    );

                    assert!(
                        v2.registry
                            .metrics
                            .iter()
                            .any(|m| &*m.name == "greenfield.requests"),
                        "greenfield metric should resolve"
                    );
                }
            }
        }
    }

    #[test]
    fn test_dep_exclusion_migration_redefine_ref_before_def() {
        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let config = WeaverResolverConfig {
                target,
                ..Default::default()
            };
            // Test that when a consumer references a redefined attribute (via ref) in a group,
            // it resolves against the local definition rather than falling back to the dependency's
            // excluded definition.
            let consumer_yaml = r#"
file_format: definition/2
metrics:
  - name: moved.metric
    requirement_level: recommended
    instrument: counter
    unit: "1"
    stability: stable
    brief: Redefined metric.
    attributes:
      - ref: moved.attr
        requirement_level: required
attributes:
  - key: moved.attr
    type: string
    stability: stable
    brief: Redefined attribute (same name as the deprecated parent item).
"#;
            let result = resolve_inline_with_parent_and_config(
                consumer_yaml,
                "data/registry-test-dep-exclusion/migration_parent/registry",
                config,
            );
            let resolved = match result {
                WResult::Ok(s) | WResult::OkWithNFEs(s, _) => s,
                WResult::FatalErr(e) => panic!("expected success; got {e:?}"),
            };

            match resolved {
                WeaverResolvedSchema::V1(v1) => {
                    let metric = v1
                        .groups(GroupType::Metric)
                        .get("metric.moved.metric")
                        .cloned()
                        .expect("metric.moved.metric should be present");
                    let attr = v1
                        .catalog
                        .attribute(&metric.attributes[0])
                        .expect("attribute should exist in catalog");
                    assert_eq!(attr.name, "moved.attr");
                }
                WeaverResolvedSchema::V2(v2) => {
                    let metric = v2
                        .registry
                        .metrics
                        .iter()
                        .find(|m| &*m.name == "moved.metric")
                        .expect("metric.moved.metric should be present");
                    let aref = &metric.attributes[0];
                    let attr = v2
                        .attribute_catalog
                        .get(aref.base.0 as usize)
                        .expect("attribute should exist in catalog");
                    assert_eq!(attr.key, "moved.attr");
                }
            }
        }
    }

    #[test]
    fn test_within_registry_leak_v2_refinement() {
        // V2: a public metric_refinement targets an excluded base metric in
        // the same registry. Exercises the `extends` exclusion path on V2.
        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let res = resolve_at_with_config(
                "data/registry-test-dep-exclusion/within_registry_v2_leak_ref",
                WeaverResolverConfig {
                    target,
                    ..Default::default()
                },
            );
            let expected_id = match target {
                ResolutionTarget::V1 => "metric.parent.base",
                ResolutionTarget::V2 => "parent.base",
            };
            assert_exclusion_errors(res, &[(expected_id, "child.refined")]);
        }
    }

    #[test]
    fn test_v2_refinement_note_inheritance() {
        for (signal, definition) in [
            (
                "span",
                "type: example.client\n    kind: client\n    name:\n      note: Use the operation name.",
            ),
            (
                "metric",
                "name: example.client\n    instrument: counter\n    unit: '1'",
            ),
            ("event", "name: example.client"),
            (
                "entity",
                "type: example.client\n    identity:\n      - ref: example.id",
            ),
        ] {
            let collection = if signal == "entity" {
                "entities".to_owned()
            } else {
                format!("{signal}s")
            };
            for (parent_note, note, expected) in [
                ("    note: Parent note.", "", Some("Parent note.")),
                ("    note: Parent note.", "    note: \"\"", Some("")),
                (
                    "    note: Parent note.",
                    "    note: Child note.",
                    Some("Child note."),
                ),
                ("", "", None),
                ("", "    note: \"\"", Some("")),
                ("    note: \"\"", "", Some("")),
            ] {
                let yaml = format!(
                    r#"
file_format: definition/2
{collection}:
  - {definition}
    requirement_level: recommended
    stability: development
    brief: Example signal.
{parent_note}
attributes:
  - key: example.id
    type: int
    stability: development
    brief: Entity identifier.
{signal}_refinements:
  - id: example.special.client
    ref: example.client
{note}
"#
                );
                let loaded = LoadedSemconvRegistry::create_from_string(&yaml).unwrap();
                let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
                let resolved = resolver
                    .resolve_loaded(loaded)
                    .into_result_failing_non_fatal()
                    .expect("Failed to resolve refinement");
                let v2: weaver_resolved_schema::v2::ResolvedTelemetrySchema =
                    Arc::unwrap_or_clone(resolved).into_v1().unwrap().try_into().unwrap();
                let json = serde_json::to_value(v2).unwrap();
                let refinement = json["refinements"][&collection]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|group| group["id"] == "example.special.client")
                    .expect("Refinement not found");
                let expected = expected.filter(|note| !note.is_empty());
                assert_eq!(
                    refinement.get("note").and_then(|value| value.as_str()),
                    expected,
                    "{signal} with parent {parent_note:?} and refinement {note:?}"
                );
                assert_eq!(refinement.get("note").is_some(), expected.is_some());
            }
        }
    }

    #[test]
    fn test_v2_refinement_attribute_precedence() {
        // Precedence for an attribute a refinement inherits from its parent,
        // re-declares via `ref_group`, and overrides inline must be:
        // inline `ref:` > `ref_group` > parent.
        //   - test.a: parent + ref_group -> ref_group wins.
        //   - test.b: parent + ref_group + inline -> inline wins.
        let registry_spec = r#"
file_format: definition/2
attributes:
  - key: test.a
    type: string
    brief: Base a
    stability: stable
  - key: test.b
    type: string
    brief: Base b
    stability: stable
attribute_groups:
  - id: parent.group
    visibility: public
    brief: Parent group
    stability: stable
    attributes:
      - ref: test.a
        brief: parent a
      - ref: test.b
        brief: parent b
  - id: override.group
    visibility: public
    brief: Override group
    stability: stable
    attributes:
      - ref: test.a
        brief: override a
      - ref: test.b
        brief: override b
spans:
  - type: base.span
    requirement_level: recommended
    kind: client
    name:
      note: base span
    brief: Base span
    stability: stable
    attributes:
      - ref_group: parent.group
span_refinements:
  - id: span.refined
    ref: base.span
    brief: Refined span
    attributes:
      - ref_group: override.group
      - ref: test.b
        brief: inline b
"#;
        let loaded = LoadedSemconvRegistry::create_from_string(registry_spec)
            .expect("Failed to load semconv spec");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let resolved = match resolver.resolve_loaded(loaded) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => {
                Arc::unwrap_or_clone(r).into_v1().unwrap()
            }
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let refined = resolved
            .group("span.refined")
            .expect("span.refined not found");
        let mut briefs = HashMap::new();
        for attr_ref in &refined.attributes {
            let attr = resolved
                .catalog
                .attribute(attr_ref)
                .expect("Failed to resolve attribute ref");
            _ = briefs.insert(attr.name.clone(), attr.brief.clone());
        }
        assert_eq!(
            briefs.get("test.a").map(String::as_str),
            Some("override a"),
            "ref_group must override the parent-inherited attribute"
        );
        assert_eq!(
            briefs.get("test.b").map(String::as_str),
            Some("inline b"),
            "inline `ref:` must override both ref_group and parent"
        );
    }

    #[test]
    fn test_v2_unresolved_ref_group() {
        // A `ref_group` pointing at a group that does not exist reports
        // `UnresolvedIncludeRef` (not the `extends`-flavored error).
        let result = create_registry_from_string(
            r#"
file_format: definition/2
spans:
  - type: base.span
    requirement_level: recommended
    kind: client
    name:
      note: base span
    brief: Base span
    stability: stable
    attributes:
      - ref_group: missing.group
"#,
        );
        match result.into_result_failing_non_fatal() {
            Ok(_) => panic!("expected an unresolved include error"),
            Err(Error::CompoundError(errors)) => {
                assert!(
                    errors.iter().any(|e| matches!(
                        e,
                        Error::UnresolvedIncludeRef { group_id, include_ref, .. }
                            if group_id == "span.base.span" && include_ref == "missing.group"
                    )),
                    "expected UnresolvedIncludeRef on span.base.span, got {errors:#?}"
                );
            }
            Err(e) => panic!("expected CompoundError, got {e:?}"),
        }
    }

    #[test]
    fn test_v2_excluded_ref_group() {
        // A public span pulling in an excluded group via `ref_group` leaks it —
        // the include path must trip the same exclusion check as `extends`.
        let result = create_registry_from_string(
            r#"
file_format: definition/2
attributes:
  - key: test.attr
    type: string
    brief: attr
    stability: stable
attribute_groups:
  - id: attrs.excluded
    visibility: public
    brief: Excluded group.
    stability: stable
    annotations:
      dependency_resolution:
        exclude: true
    attributes:
      - ref: test.attr
spans:
  - type: leaky.span
    requirement_level: recommended
    kind: internal
    name:
      note: leaky
    brief: Public span that pulls in an excluded group.
    stability: stable
    attributes:
      - ref_group: attrs.excluded
"#,
        );
        match result.into_result_failing_non_fatal() {
            Ok(_) => panic!("expected an exclusion error"),
            Err(Error::CompoundError(errors)) => {
                assert!(
                    errors.iter().any(|e| matches!(
                        e,
                        Error::ExcludedFromDependencyResolution { id, used_in, .. }
                            if id == "attrs.excluded" && used_in == "span.leaky.span"
                    )),
                    "expected ExcludedFromDependencyResolution on span.leaky.span, got {errors:#?}"
                );
            }
            Err(e) => panic!("expected CompoundError, got {e:?}"),
        }
    }

    #[test]
    fn test_within_registry_internal_group_with_inline_excluded_attr() {
        // An `attribute_group` with `visibility: internal` is dropped before
        // the resolved schema is emitted, so an excluded inline attribute on
        // it cannot leak. The same-registry leak check must therefore treat
        // an internal group like an excluded one.
        let result = create_registry_from_string(
            "
groups:
    - id: attrs.internal
      type: attribute_group
      brief: Internal-only group.
      visibility: internal
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true",
        );

        if let Err(e) = result.into_result_failing_non_fatal() {
            panic!("expected success for internal group with inline excluded attr; got {e:?}");
        }
    }

    #[test]
    fn test_within_registry_internal_group_extends_excluded_parent() {
        // Internal consumer extending an excluded parent group must also be
        // exempt — same rationale as the inline-attr case, but exercised via
        // the `extends` (excluded_parent_error) path.
        let result = create_registry_from_string(
            "
groups:
    - id: attrs.private
      type: attribute_group
      brief: Private
      annotations:
        dependency_resolution:
          exclude: true
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true
    - id: attrs.internal_consumer
      type: attribute_group
      brief: Internal group extending an excluded parent.
      visibility: internal
      extends: attrs.private",
        );

        if let Err(e) = result.into_result_failing_non_fatal() {
            panic!("expected success for internal group extending excluded parent; got {e:?}");
        }
    }

    #[test]
    fn test_within_registry_internal_group_transitive_leak_still_fails() {
        // Internal groups exempt themselves, not their consumers. A public
        // span that pulls the internal group in via `include_groups` inherits
        // the excluded attribute ref and must still trip the leak check.
        let result = create_registry_from_string(
            "
groups:
    - id: attrs.internal
      type: attribute_group
      brief: Internal-only group hosting an excluded attribute.
      visibility: internal
      attributes:
        - id: secret.value
          type: string
          stability: stable
          brief: Hidden detail.
          examples: ['hidden']
          annotations:
            dependency_resolution:
              exclude: true
    - id: span.leaks
      type: span
      span_kind: internal
      stability: stable
      brief: Public span that transitively pulls in an excluded attribute.
      extends: attrs.internal",
        );

        match result.into_result_failing_non_fatal() {
            Ok(_) => panic!("expected an exclusion error on span.leaks"),
            Err(Error::CompoundError(errors)) => {
                assert!(
                    errors.iter().any(|e| matches!(
                        e,
                        Error::ExcludedFromDependencyResolution { id, used_in, .. }
                            if id == "secret.value" && used_in == "span.leaks"
                    )),
                    "expected exclusion error on span.leaks, got {errors:#?}"
                );
            }
            Err(e) => panic!("expected CompoundError, got {e:?}"),
        }
    }

    #[test]
    fn test_diamond_dependency_attribute_conflict() -> Result<(), weaver_semconv::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/diamond-conflict/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let config = WeaverResolverConfig::default();
        let mut resolver = WeaverResolver::new(config);

        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        // Verify that conflict_attr from registry_c v1.2 was selected
        // because it has a higher version than v1.1.
        let (attr, _) = resolved
            .catalog
            .root_attribute("conflict_attr")
            .expect("conflict_attr not found in catalog");

        assert_eq!(attr.brief, "Attribute from C v1.2");
        Ok(())
    }

    #[test]
    fn test_imported_attribute_keeps_origin_provenance() -> Result<(), weaver_semconv::Error> {
        // `main` imports one metric from `base` and one from `middle`; both
        // use `host.id`, which is defined only in `base` (a dependency of
        // `middle`). Middle's re-export of `host.id` must keep base's
        // provenance so both paths agree on the attribute's origin and the
        // schema resolves cleanly.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/correct-provenance/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let config = WeaverResolverConfig::default();
        let mut resolver = WeaverResolver::new(config);

        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let (attr, _) = resolved
            .catalog
            .root_attribute("host.id")
            .expect("host.id not found in catalog");
        assert_eq!(attr.brief, "Unique host ID.");
        Ok(())
    }

    #[test]
    fn test_upgraded_group_keeps_winning_version_provenance() -> Result<(), weaver_semconv::Error> {
        // `main` imports `c.metric.1` via A (which pins C v1.1) and
        // `c.span.2` via B (which pins C v1.2). The compatible version
        // conflict resolves in favor of C v1.2 and the attribute catalog is
        // upgraded accordingly (`c.attr1` reports the v1.2 brief, as covered
        // by test_standalone_vs_graph_provenance_immutability). The group
        // must be upgraded consistently: `metric.c.metric.1` should carry
        // C v1.2's definition and provenance.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/compatible-version-conflict/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let config = WeaverResolverConfig::default();
        let mut resolver = WeaverResolver::new(config);

        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let metrics = resolved.groups(GroupType::Metric);
        let metric = metrics
            .get("metric.c.metric.1")
            .expect("metric.c.metric.1 not found");
        let provenance = metric
            .provenance()
            .expect("metric.c.metric.1 has no lineage");
        assert_eq!(
            provenance.schema_url.to_string(),
            "https://example.com/c/1.2.0"
        );
        assert_eq!(metric.brief, "Metric 1 in C v1.2 (updated)");
        Ok(())
    }

    #[test]
    fn test_upgraded_group_keeps_winning_version_provenance_published(
    ) -> Result<(), weaver_semconv::Error> {
        // Same shape as test_upgraded_group_keeps_winning_version_provenance,
        // but A is a *published* (resolved/2.0) registry whose embedded copy
        // of `c.metric.1` was resolved against C v1.1. B (a definition
        // registry) pins C v1.2, which wins the version conflict. The metric
        // imported through the published V2 path must also be upgraded to
        // C v1.2's definition and provenance.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/compatible-version-conflict-published/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let config = WeaverResolverConfig::default();
        let mut resolver = WeaverResolver::new(config);

        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let metrics = resolved.groups(GroupType::Metric);
        let metric = metrics
            .values()
            .find(|g| g.metric_name.as_deref() == Some("c.metric.1"))
            .expect("metric c.metric.1 not found");
        let provenance = metric.provenance().expect("c.metric.1 has no lineage");
        assert_eq!(
            provenance.schema_url.to_string(),
            "https://example.com/c/1.2.0"
        );
        assert_eq!(metric.brief, "Metric 1 in C v1.2 (updated)");
        Ok(())
    }

    #[test]
    fn test_v1_manifest_resolves_dependency_declared_by_name() {
        // `main` is a v1 manifest, where `name` + `registry_path` stays supported.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/v1-dependency-syntax/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("a v1 manifest may declare dependencies by name");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };
        assert!(
            resolved
                .groups(GroupType::Metric)
                .values()
                .any(|g| g.metric_name.as_deref() == Some("dep.uptime")),
            "expected the imported metric from the dependency to resolve"
        );
    }

    #[test]
    fn test_dependency_without_schema_url_is_rejected() {
        // `main` is a v2 manifest (it declares its own `schema_url`) but declares its
        // dependency with only `name` and `registry_path`. `schema_url` is mandatory there --
        // it is the identity provenance and version-conflict resolution key on.
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/mandatory-schema-url/main".to_owned(),
        };
        let err_msg = match RegistryRepo::try_new(None, &registry_path, &mut vec![]) {
            Err(e) => e.to_string(),
            Ok(registry_repo) => {
                let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
                match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
                    WResult::Ok(_) | WResult::OkWithNFEs(_, _) => panic!(
                        "expected an error for the dependency missing 'schema_url', but resolution succeeded"
                    ),
                    WResult::FatalErr(e) => e.to_string(),
                }
            }
        };
        assert!(
            err_msg.contains("schema_url") && err_msg.contains("dep"),
            "error should name the dependency missing 'schema_url'; got: {err_msg}"
        );
    }

    #[test]
    fn test_standalone_vs_graph_provenance_immutability() -> Result<(), weaver_semconv::Error> {
        // 1. Setup a single WeaverResolver instance with overrides pointing to compatible-version-conflict.
        let mut config = WeaverResolverConfig::default();
        _ = config.schema_url_overrides.insert(
            SchemaUrl::try_from("https://example.com/a/0.1.0").unwrap(),
            VirtualDirectoryPath::LocalFolder {
                path: "data/compatible-version-conflict/registry_a".to_owned(),
            },
        );
        _ = config.schema_url_overrides.insert(
            SchemaUrl::try_from("https://example.com/b/0.1.0").unwrap(),
            VirtualDirectoryPath::LocalFolder {
                path: "data/compatible-version-conflict/registry_b".to_owned(),
            },
        );
        let url_c_v1_1 = SchemaUrl::try_from("https://example.com/c/1.1.0").unwrap();
        _ = config.schema_url_overrides.insert(
            url_c_v1_1.clone(),
            VirtualDirectoryPath::LocalFolder {
                path: "data/compatible-version-conflict/registry_c_v1_1".to_owned(),
            },
        );
        let url_c_v1_2 = SchemaUrl::try_from("https://example.com/c/1.2.0").unwrap();
        _ = config.schema_url_overrides.insert(
            url_c_v1_2.clone(),
            VirtualDirectoryPath::LocalFolder {
                path: "data/compatible-version-conflict/registry_c_v1_2".to_owned(),
            },
        );

        let mut resolver = WeaverResolver::new(config);

        // 2. Resolve a/0.1.0 in isolation first.
        let url_a = SchemaUrl::try_from("https://example.com/a/0.1.0").unwrap();
        let resolved_a = match resolver.resolve_schema(&url_a) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Failed to resolve standalone A: {e}"),
        };

        // Standalone A should have c.attr1 from C v1.1.
        let v1_a = resolved_a
            .as_v1()
            .expect("Expected V1 schema for standalone A");
        let (attr_a, source_a) = v1_a
            .catalog
            .root_attribute("c.attr1")
            .expect("c.attr1 not found in standalone A");
        assert_eq!(attr_a.brief, "Attribute 1 from C v1.1");
        assert_eq!(source_a, "v2_dependency.example.com/c");
        assert!(v1_a.dependencies.contains(&url_c_v1_1));

        // 3. Now resolve main, which imports from both A and B (with B bringing C v1.2).
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/compatible-version-conflict/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let resolved_main =
            match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
                WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
                WResult::FatalErr(e) => panic!("Failed to resolve main: {e}"),
            };

        // Main should upgrade c.attr1 to C v1.2 on-the-fly via cache lookup.
        let v1_main = resolved_main.as_v1().expect("Expected V1 schema for main");
        let (attr_main, source_main) = v1_main
            .catalog
            .root_attribute("c.attr1")
            .expect("c.attr1 not found in main");
        assert_eq!(attr_main.brief, "Attribute 1 from C v1.2 (updated)");
        assert_eq!(source_main, "v2_dependency.example.com/c");
        let url_b = SchemaUrl::try_from("https://example.com/b/0.1.0").unwrap();
        assert!(v1_main.dependencies.contains(&url_a));
        assert!(v1_main.dependencies.contains(&url_b));
        assert!(
            !v1_main.dependencies.contains(&url_c_v1_1),
            "superseded dependency C v1.1 should be excluded from linearized dependencies {:?}",
            v1_main.dependencies
        );
        assert!(
            v1_main.dependencies.contains(&url_c_v1_2),
            "chosen dependency C v1.2 missing from {:?}",
            v1_main.dependencies
        );

        // 4. Check the cached A schema in isolation once again.
        // It MUST remain completely untouched and still report C v1.1.
        let resolved_a_again = match resolver.resolve_schema(&url_a) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Failed to resolve standalone A second time: {e}"),
        };
        let v1_a_again = resolved_a_again
            .as_v1()
            .expect("Expected V1 schema for cached A");
        let (attr_a_again, source_a_again) = v1_a_again
            .catalog
            .root_attribute("c.attr1")
            .expect("c.attr1 not found in cached A");
        assert_eq!(attr_a_again.brief, "Attribute 1 from C v1.1");
        assert_eq!(source_a_again, "v2_dependency.example.com/c");
        assert!(v1_a_again.dependencies.contains(&url_c_v1_1));

        Ok(())
    }

    /// An element reaching a registry through a dependency-of-a-dependency must
    /// be attributed to the registry that defined it.
    ///
    /// The `transitive-provenance` fixture is a plain three-level chain:
    /// `base <- middle <- main`, where `middle` only re-exports base's metric and
    /// `main` depends on `middle` alone. `base` is therefore reachable solely
    /// through `middle`.
    ///
    /// Provenance is a `DependencyRef` index into the resolved schema's
    /// `dependencies` set. If that set records only the direct dependencies,
    /// `base` has no entry for the metric to point at, its `source` is left
    /// empty, and the metric is reported as defined by `main` itself — even
    /// though its recorded path names a file in `base`.
    #[test]
    fn test_transitive_dependency_provenance_resolves() -> Result<(), Error> {
        use weaver_resolved_schema::v2::catalog::AttributeCatalog;

        const BASE: &str = "https://example.com/base/1.0.0";

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/transitive-provenance/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("failed to create registry repo");
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Failed to resolve main: {e}"),
        };
        let v1 = resolved.as_v1().expect("Expected a V1 schema for main");
        let v2: weaver_resolved_schema::v2::ResolvedTelemetrySchema =
            v1.clone().try_into().expect("v1 -> v2 conversion");

        // `base` is not a direct dependency of `main`, so it is only in the set
        // if transitive dependencies are recorded.
        let base_url: SchemaUrl = BASE.try_into().expect("valid schema url");
        assert!(
            v2.dependencies.contains(&base_url),
            "transitive dependency `base` missing from {:?}",
            v2.dependencies
        );

        let deps: Vec<&SchemaUrl> = v2.dependencies.iter().collect();
        let metric = v2
            .registry
            .metrics
            .iter()
            .find(|m| &*m.name == "base.uptime")
            .expect("main imported base.uptime");

        assert!(
            !metric.provenance.is_empty(),
            "`base.uptime` came from `base` but is reported as defined by `main`"
        );
        let source = metric
            .provenance
            .source
            .and_then(|r| deps.get(r.0 as usize))
            .expect("metric provenance resolves to a dependency");
        assert_eq!(source.as_str(), BASE, "attributed to the wrong registry");

        // The attribute it carries came from `base` too, not from `middle`.
        let attr_ref = metric
            .attributes
            .first()
            .expect("base.uptime has attributes");
        let attr = v2
            .attribute_catalog
            .attribute(&attr_ref.base)
            .expect("attribute in catalog");
        let attr_source = attr
            .provenance
            .source
            .and_then(|r| deps.get(r.0 as usize))
            .expect("attribute provenance resolves to a dependency");
        assert_eq!(attr_source.as_str(), BASE);

        Ok(())
    }

    /// True for the "`definition/2` is not yet stable" warning. Every file in
    /// these fixtures gives this warning, and these tests ignore it.
    fn is_unstable_format_warning(e: &Error) -> bool {
        matches!(
            e,
            Error::FailToResolveDefinition(weaver_semconv::Error::UnstableFileFormat { .. })
        )
    }

    fn load_entity_assoc_fixture_with_engine(
        name: &str,
        target: ResolutionTarget,
    ) -> WResult<WeaverResolvedSchema, Error> {
        let candidate = format!("data/entity-assoc-imports/{name}/registry");
        let path = if std::path::Path::new(&candidate).is_dir() {
            candidate
        } else {
            format!("data/entity-assoc-imports/{name}")
        };
        let registry_path = VirtualDirectoryPath::LocalFolder { path };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("failed to create registry repo");
        let config = WeaverResolverConfig {
            target,
            ..Default::default()
        };
        WeaverResolver::new(config).load_and_resolve_schema(registry_repo, DefaultSchemaVisitor)
    }

    fn load_entity_assoc_fixture(name: &str) -> WResult<WeaverResolvedSchema, Error> {
        load_entity_assoc_fixture_with_engine(name, ResolutionTarget::V1)
    }

    /// Resolves one of the `entity-assoc-imports` registries. Returns the
    /// resolved schema and the non-fatal errors.
    fn resolve_entity_assoc_fixture_with_engine(
        name: &str,
        target: ResolutionTarget,
    ) -> (V2Schema, Vec<Error>) {
        let (resolved, nfes) = match load_entity_assoc_fixture_with_engine(name, target) {
            WResult::Ok(r) => (r, vec![]),
            WResult::OkWithNFEs(r, nfes) => (r, nfes),
            WResult::FatalErr(e) => panic!("Failed to resolve `{name}`: {e}"),
        };
        let v2: V2Schema = match resolved {
            WeaverResolvedSchema::V2(v2) => v2,
            WeaverResolvedSchema::V1(v1) => v1
                .try_into()
                .unwrap_or_else(|e| panic!("v1 -> v2 conversion for `{name}`: {e}")),
        };
        let nfes = nfes
            .into_iter()
            .filter(|e| !is_unstable_format_warning(e))
            .filter(|e| !matches!(e, Error::V1DependencyUsesV1Path { .. }))
            .collect();
        (v2, nfes)
    }

    fn resolve_entity_assoc_fixture(name: &str) -> (V2Schema, Vec<Error>) {
        resolve_entity_assoc_fixture_with_engine(name, ResolutionTarget::V1)
    }

    fn load_signal_name_fixture_with_engine(
        name: &str,
        target: ResolutionTarget,
    ) -> WeaverResolvedSchema {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: format!("data/signal-name-preservation/{name}/registry"),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("failed to create registry repo");
        let config = WeaverResolverConfig {
            target,
            ..Default::default()
        };
        match WeaverResolver::new(config)
            .load_and_resolve_schema(registry_repo, DefaultSchemaVisitor)
        {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
            WResult::FatalErr(e) => panic!("Failed to resolve `{name}`: {e}"),
        }
    }

    /// Asserts the materialized v2 signal names/types match the expected values.
    fn check_v2_signal_names(v2: &V2Schema, entity: &str, metric: &str, event: &str, span: &str) {
        let entities: Vec<String> = v2
            .registry
            .entities
            .iter()
            .map(|e| e.r#type.to_string())
            .collect();
        let metrics: Vec<String> = v2
            .registry
            .metrics
            .iter()
            .map(|m| m.name.to_string())
            .collect();
        let events: Vec<String> = v2
            .registry
            .events
            .iter()
            .map(|e| e.name.to_string())
            .collect();
        let spans: Vec<String> = v2
            .registry
            .spans
            .iter()
            .map(|s| s.r#type.to_string())
            .collect();
        let mut wrong = Vec::new();
        for (label, want, got) in [
            ("entity type", entity, &entities),
            ("metric name", metric, &metrics),
            ("event name", event, &events),
            ("span type", span, &spans),
        ] {
            if got.len() != 1 || got.first().map(String::as_str) != Some(want) {
                wrong.push(format!("{label}: want {want:?}, got {got:?}"));
            }
        }
        assert!(
            wrong.is_empty(),
            "unexpected v2 signal names -> {}",
            wrong.join("; ")
        );
    }

    /// A v2 (`definition/2`) registry names a signal directly, with no id
    /// prefix, so the materialized name is kept verbatim.
    #[test]
    fn test_v2_schema_preserves_signal_names() {
        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let resolved = load_signal_name_fixture_with_engine("v2", target);
            let v2: V2Schema = match resolved {
                WeaverResolvedSchema::V2(v2) => v2,
                WeaverResolvedSchema::V1(v1) => v1.try_into().expect("v1 -> v2 conversion"),
            };
            check_v2_signal_names(&v2, "entity.test", "metric.test", "event.test", "span.test");
        }
    }

    #[test]
    fn test_entity_assoc_fixtures_dual_engine_parity() {
        let fixtures = [
            "local_shapes",
            "local_private_pair",
            "middle",
            "middle_reexport",
            "middle_refined_signal",
            "middle_refinement_assoc",
            "top_diamond",
            "top_diamond_assoc",
            "top_private_first",
            "top_public_first",
            "top_rebind",
            "top_legacy_import",
            "legacy_resource",
        ];
        for fixture in fixtures {
            let (legacy_schema, legacy_nfes) =
                resolve_entity_assoc_fixture_with_engine(fixture, ResolutionTarget::V1);
            let (native_schema, native_nfes) =
                resolve_entity_assoc_fixture_with_engine(fixture, ResolutionTarget::V2);
            let legacy_schema_val = serde_json::to_value(&legacy_schema).unwrap();
            let native_schema_val = serde_json::to_value(&native_schema).unwrap();
            assert_eq!(
                legacy_schema_val, native_schema_val,
                "Schema mismatch between Legacy and Native for `{fixture}`"
            );
            assert_eq!(
                legacy_nfes.len(),
                native_nfes.len(),
                "NFEs count mismatch between Legacy and Native for `{fixture}`"
            );
            for nfe in &legacy_nfes {
                match nfe {
                    Error::DuplicateGroupId { group_id, .. } => {
                        assert!(
                            native_nfes.iter().any(|e| matches!(e, Error::DuplicateGroupId { group_id: gid, .. } if gid == group_id)),
                            "Native missing DuplicateGroupId for {group_id}"
                        );
                    }
                    Error::DuplicateGroupName { group_name, .. } => {
                        assert!(
                            native_nfes.iter().any(|e| matches!(e, Error::DuplicateGroupName { group_name: gname, .. } if gname == group_name)),
                            "Native missing DuplicateGroupName for {group_name}"
                        );
                    }
                    _ => {
                        let nfe_json = serde_json::to_string(nfe).unwrap();
                        assert!(
                            native_nfes
                                .iter()
                                .any(|e| serde_json::to_string(e).unwrap() == nfe_json),
                            "Native missing error {nfe_json} for {fixture}"
                        );
                    }
                }
            }
        }

        let fatal_fixtures = [
            "middle_bad_assoc",
            "middle_excluded_assoc",
            "top_ambiguous_assoc",
            "top_transitive_assoc",
            "middle_name_clash",
            "legacy_by_id",
            "local_private_assoc",
        ];
        for fixture in fatal_fixtures {
            let legacy_res = load_entity_assoc_fixture_with_engine(fixture, ResolutionTarget::V1);
            let native_res = load_entity_assoc_fixture_with_engine(fixture, ResolutionTarget::V2);
            assert!(
                matches!(legacy_res, WResult::FatalErr(_)),
                "Expected legacy failure for {fixture}"
            );
            assert!(
                matches!(native_res, WResult::FatalErr(_)),
                "Expected native failure for {fixture}"
            );
        }
    }

    /// The non-fatal errors of one of the `entity-assoc-imports` registries.
    fn entity_assoc_fixture_errors(name: &str) -> Vec<Error> {
        resolve_entity_assoc_fixture(name).1
    }

    /// The errors of an `entity-assoc-imports` registry that must not resolve.
    fn entity_assoc_fixture_fatal_errors(name: &str) -> Vec<Error> {
        match load_entity_assoc_fixture(name) {
            WResult::FatalErr(Error::CompoundError(errors)) => errors,
            WResult::FatalErr(e) => vec![e],
            _ => panic!("`{name}` must fail to resolve"),
        }
    }

    fn entity_assoc_fixture_fatal_errors_with_engine(
        name: &str,
        target: ResolutionTarget,
    ) -> Vec<Error> {
        match load_entity_assoc_fixture_with_engine(name, target) {
            WResult::FatalErr(Error::CompoundError(errors)) => errors,
            WResult::FatalErr(e) => vec![e],
            _ => panic!("`{name}` must fail to resolve"),
        }
    }

    /// The association expressions of the one metric in a fixture registry.
    fn metric_associations(
        schema: &V2Schema,
    ) -> &[weaver_resolved_schema::v2::entity::EntityAssociation] {
        let [metric] = schema.registry.metrics.as_slice() else {
            panic!("expected one metric, got {:?}", schema.registry.metrics);
        };
        &metric.entity_associations
    }

    /// The schema url an entity reference points at. Returns `None` when the
    /// reference names an entity of this registry.
    fn ref_source<'a>(
        schema: &'a V2Schema,
        entity_ref: &weaver_resolved_schema::v2::entity::EntityRef,
    ) -> Option<&'a str> {
        let deps: Vec<&SchemaUrl> = schema.dependencies.iter().collect();
        entity_ref
            .provenance
            .source
            .and_then(|r| deps.get(r.0 as usize))
            .map(|url| url.as_str())
    }

    /// The registry that defines the `host` entity.
    const BASE_URL: &str = "https://example.com/base/1.0.0";

    /// Entity types in the resolved registry, in registry order.
    fn entity_types(schema: &V2Schema) -> Vec<&str> {
        schema
            .registry
            .entities
            .iter()
            .map(|e| e.r#type.as_ref())
            .collect()
    }

    /// The schema url of the registry that defines an entity. Returns `None`
    /// when the resolver reports the entity as local to this registry.
    fn entity_source<'a>(schema: &'a V2Schema, entity_type: &str) -> Option<&'a str> {
        let deps: Vec<&SchemaUrl> = schema.dependencies.iter().collect();
        let entity = schema
            .registry
            .entities
            .iter()
            .find(|e| &*e.r#type == entity_type)
            .expect("entity is in the resolved registry");
        entity
            .provenance
            .source
            .and_then(|r| deps.get(r.0 as usize))
            .map(|url| url.as_str())
    }

    /// A dependent registry must import a legacy `type: resource` entity. The
    /// group has no name, so a pattern can use only the id. The id holds the
    /// entity type.
    #[test]
    fn test_legacy_resource_entity_can_be_imported() {
        let (schema, nfes) = resolve_entity_assoc_fixture("top_legacy_import");
        assert!(nfes.is_empty(), "unexpected errors: {nfes:?}");
        assert_eq!(
            entity_types(&schema),
            ["browser"],
            "the import must reach the legacy resource group of the dependency"
        );
    }

    /// A definition that two dependency paths reach must be imported one time.
    ///
    /// `top_diamond` reaches `host` directly from `base`, and also through
    /// `middle_reexport`. Both paths lead to the same definition in `base`.
    /// There is no conflict to resolve and nothing to report. The entity must
    /// appear one time only, with `base` as its source.
    #[test]
    fn test_entity_reachable_by_two_paths_is_imported_once() {
        let (top, nfes) = resolve_entity_assoc_fixture("top_diamond");

        assert_eq!(
            entity_types(&top),
            ["host"],
            "two paths reach one definition in `base`, so the result is still \
             one entity"
        );
        assert_eq!(entity_source(&top, "host"), Some(BASE_URL));

        let duplicates: Vec<&Error> = nfes
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Error::DuplicateGroupId { .. } | Error::DuplicateGroupName { .. }
                )
            })
            .collect();
        assert!(
            duplicates.is_empty(),
            "an import of one definition by two paths is not a duplicate declaration: {duplicates:?}"
        );

        // The imported metric brings an association that resolved to the `host`
        // of `base`, and this registry imported that same definition. The two
        // are one entity, so the reference stays local rather than pointing back
        // out at a definition the registry already holds.
        let [weaver_resolved_schema::v2::entity::EntityAssociation::Ref(host)] =
            metric_associations(&top)
        else {
            panic!(
                "expected one reference, got {:?}",
                metric_associations(&top)
            );
        };
        assert_eq!(&*host.r#type, "host");
        assert_eq!(ref_source(&top, host), None);
    }

    /// An import that no dependency satisfies must be reported.
    ///
    /// `top_bad_import` depends on `middle_reexport`, which does export `host`.
    /// The import asks for `no.such.entity`. So the import fails for one reason
    /// only: no dependency offers anything with that name.
    #[test]
    fn test_import_that_binds_to_nothing_is_reported() {
        let errors = entity_assoc_fixture_errors("top_bad_import");
        assert!(
            matches!(
                errors.as_slice(),
                [Error::UnmatchedImport { pattern, signal }]
                    if pattern == "no.such.entity" && signal == "entities"
            ),
            "no dependency offers `no.such.entity`, and the resolver must report \
             this import: {errors:?}"
        );
    }

    /// Every `imports` field is checked, not only `entities`.
    ///
    /// `top_bad_imports_all_types` asks for one name of each signal type that
    /// no dependency offers. It also asks for a metric and an entity that
    /// `middle_reexport` does export, so a report of those would be wrong.
    #[test]
    fn test_unmatched_imports_are_reported_for_every_signal_type() {
        let errors = entity_assoc_fixture_errors("top_bad_imports_all_types");
        let mut reported: Vec<(&str, &str)> = errors
            .iter()
            .map(|e| match e {
                Error::UnmatchedImport { pattern, signal } => (pattern.as_str(), signal.as_str()),
                other => panic!("unexpected error: {other:?}"),
            })
            .collect();
        reported.sort_unstable();
        assert_eq!(
            reported,
            [
                ("no.such.attribute_group", "attribute_groups"),
                ("no.such.entity", "entities"),
                ("no.such.event", "events"),
                ("no.such.metric", "metrics"),
                ("no.such.span", "spans"),
            ],
            "each signal type must report its own unmatched import, and the \
             imports that do bind must stay silent"
        );
    }

    /// Two unrelated dependencies that each declare their own group with the
    /// same id are a genuine conflict, not a diamond.
    ///
    /// `base` and `rival_base` share no history: different origin registries,
    /// different identity attributes, one `host` entity each. Deduplication
    /// buckets candidates by origin, so it must not collapse these two into
    /// one — picking a winner silently would drop a definition the registry
    /// asked for. Both are imported and the clash is reported, so the author
    /// can resolve it.
    #[test]
    fn test_same_entity_from_unrelated_dependencies_is_reported() {
        let (top, nfes) = resolve_entity_assoc_fixture("top_rival_import");

        assert_eq!(
            entity_types(&top),
            ["host", "host"],
            "unrelated definitions must not be silently deduplicated"
        );

        let duplicates: Vec<&Error> = nfes
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Error::DuplicateGroupId { .. } | Error::DuplicateGroupName { .. }
                )
            })
            .collect();
        assert_eq!(
            duplicates.len(),
            2,
            "the clash must be reported by id and by name: {nfes:?}"
        );
    }

    /// An association resolves against a dependency without importing the
    /// entity, and the reference says where the definition lives.
    ///
    /// `middle` defines `service`, and names both `service` and the `host` of
    /// `base` in one `all_of`. The tree keeps its shape. The local leaf carries
    /// no provenance, the leaf from `base` names `base`, and `host` stays in
    /// `base`: there is one definition, in one place.
    #[test]
    fn test_association_resolves_against_a_dependency_without_importing() {
        use weaver_resolved_schema::v2::entity::EntityAssociation;

        let (middle, nfes) = resolve_entity_assoc_fixture("middle");
        assert!(nfes.is_empty(), "unexpected errors: {nfes:?}");
        assert_eq!(
            entity_types(&middle),
            ["service"],
            "an association must not import the entity it names"
        );

        let [EntityAssociation::AllOf { all_of }] = metric_associations(&middle) else {
            panic!(
                "expected one `all_of`, got {:?}",
                metric_associations(&middle)
            );
        };
        let [EntityAssociation::Ref(service), EntityAssociation::Ref(host)] = all_of.as_slice()
        else {
            panic!("expected two references, got {all_of:?}");
        };
        assert_eq!(&*service.r#type, "service");
        assert_eq!(
            ref_source(&middle, service),
            None,
            "an entity of this registry has no provenance"
        );
        assert_eq!(&*host.r#type, "host");
        assert_eq!(
            ref_source(&middle, host),
            Some(BASE_URL),
            "the reference must name the registry that defines `host`"
        );
    }

    /// A legacy `resource` group holds its entity type in `name`, and its id
    /// carries a `resource.` prefix that is no part of the type. Every such
    /// group in semconv v1.33.0 has this shape, and each one is named by its
    /// type in an association.
    #[test]
    fn test_association_names_a_legacy_resource_entity_by_its_name() {
        use weaver_resolved_schema::v2::entity::EntityAssociation;

        let (schema, nfes) = resolve_entity_assoc_fixture("legacy_resource");
        assert!(nfes.is_empty(), "unexpected errors: {nfes:?}");

        let mut types = entity_types(&schema);
        types.sort_unstable();
        assert_eq!(
            types,
            ["browser", "device"],
            "the entity type comes from `name`, and falls back to the id when \
             the group has none"
        );

        let device = schema
            .registry
            .metrics
            .iter()
            .find(|m| &*m.name == "legacy.device.count")
            .expect("the metric is in the resolved registry");
        let [EntityAssociation::Ref(entity_ref)] = device.entity_associations.as_slice() else {
            panic!(
                "expected one reference, got {:?}",
                device.entity_associations
            );
        };
        assert_eq!(&*entity_ref.r#type, "device");
        assert_eq!(ref_source(&schema, entity_ref), None);
    }

    /// An import satisfies an association, and the entity is then local.
    #[test]
    fn test_association_to_an_imported_entity_is_local() {
        use weaver_resolved_schema::v2::entity::EntityAssociation;

        let (middle, nfes) = resolve_entity_assoc_fixture("middle_reexport");
        assert!(nfes.is_empty(), "unexpected errors: {nfes:?}");
        assert_eq!(entity_types(&middle), ["host"]);

        let [EntityAssociation::Ref(host)] = metric_associations(&middle) else {
            panic!(
                "expected one reference, got {:?}",
                metric_associations(&middle)
            );
        };
        assert_eq!(&*host.r#type, "host");
        assert_eq!(
            ref_source(&middle, host),
            None,
            "the import put `host` in this registry, so the reference is local"
        );
    }

    /// An association that no registry in scope satisfies fails the resolve, as
    /// an attribute `ref` that resolves to nothing does.
    #[test]
    fn test_association_that_nothing_defines_fails_the_resolve() {
        let errors = entity_assoc_fixture_fatal_errors("middle_bad_assoc");
        assert!(
            matches!(
                errors.as_slice(),
                [Error::UnresolvedEntityAssociation { entity_type, .. }]
                    if entity_type == "no.such.entity"
            ),
            "nothing defines `no.such.entity`: {errors:?}"
        );
    }

    /// A private entity is out of reach by every route, an association
    /// included.
    #[test]
    fn test_association_to_a_private_entity_fails_the_resolve() {
        let legacy_errors = entity_assoc_fixture_fatal_errors_with_engine(
            "middle_excluded_assoc",
            ResolutionTarget::V1,
        );
        assert!(
            matches!(
                legacy_errors.as_slice(),
                [Error::ExcludedFromDependencyResolution { id, used_in, .. }]
                    if id == "host" && used_in == "metric.middle.request.count"
            ),
            "`base_excluded` keeps `host` private under legacy: {legacy_errors:?}"
        );

        let native_errors = entity_assoc_fixture_fatal_errors_with_engine(
            "middle_excluded_assoc",
            ResolutionTarget::V2,
        );
        assert!(
            matches!(
                native_errors.as_slice(),
                [Error::UnresolvedEntityAssociation { entity_type, .. }]
                    if entity_type == "host"
            ),
            "`base_excluded` hides `host` across dependency boundary under native: {native_errors:?}"
        );
    }

    /// An association that resolved in one registry keeps pointing at the
    /// definition it found when a consumer imports the signal.
    ///
    /// `middle_assoc_export` associates its metric with the `host` of `base`.
    /// `top_rebind` imports the metric, and defines an unrelated entity that
    /// happens to be called `host` too. The import carries a resolved
    /// reference, so it must still name `base`. Re-reading it as a bare name
    /// would silently re-point the metric at the local entity, which is a
    /// different entity with different identity attributes.
    #[test]
    fn test_imported_association_keeps_the_entity_it_resolved_to() {
        use weaver_resolved_schema::v2::entity::EntityAssociation;

        let (top, nfes) = resolve_entity_assoc_fixture("top_rebind");
        assert!(nfes.is_empty(), "unexpected errors: {nfes:?}");

        let [EntityAssociation::Ref(host)] = metric_associations(&top) else {
            panic!(
                "expected one reference, got {:?}",
                metric_associations(&top)
            );
        };
        assert_eq!(&*host.r#type, "host");
        assert_eq!(
            ref_source(&top, host),
            Some(BASE_URL),
            "the imported metric is associated with the `host` of base, not \
             with the unrelated `host` this registry defines"
        );
    }

    /// `top_private_first` and `top_public_first` list the same two
    /// dependencies in opposite orders: one excludes `host` and the other
    /// exports it, so both resolve to the exported entity.
    #[test]
    fn test_private_entity_does_not_shadow_a_public_one() {
        use weaver_resolved_schema::v2::entity::EntityAssociation;

        const RIVAL_URL: &str = "https://rival.example.com/rival/1.0.0";

        for fixture in ["top_private_first", "top_public_first"] {
            let (top, nfes) = resolve_entity_assoc_fixture(fixture);
            assert!(
                nfes.is_empty(),
                "unexpected errors in `{fixture}`: {nfes:?}"
            );

            let [EntityAssociation::Ref(host)] = metric_associations(&top) else {
                panic!(
                    "expected one reference in `{fixture}`, got {:?}",
                    metric_associations(&top)
                );
            };
            assert_eq!(&*host.r#type, "host");
            assert_eq!(
                ref_source(&top, host),
                Some(RIVAL_URL),
                "`{fixture}` must reach the one dependency that publishes \
                 `host`, whichever order the two are listed in"
            );
        }
    }

    /// `top_ambiguous_assoc` depends on `base` and `rival_base` without
    /// importing either, so an association naming `host` matches two unrelated
    /// entity definitions.
    #[test]
    fn test_association_that_two_dependencies_answer_is_reported() {
        let errors = entity_assoc_fixture_fatal_errors("top_ambiguous_assoc");
        let [Error::AmbiguousEntityAssociation {
            entity_type,
            registries,
            ..
        }] = errors.as_slice()
        else {
            panic!("expected one ambiguity, got {errors:?}");
        };
        assert_eq!(entity_type, "host");
        let mut registries = registries.clone();
        registries.sort();
        assert_eq!(
            registries,
            [BASE_URL, "https://rival.example.com/rival/1.0.0"],
            "the report must name every registry that declares the entity"
        );
    }

    /// One definition reached by two paths is not ambiguous.
    ///
    /// `top_diamond_assoc` reaches `host` through `middle_reexport` and again
    /// through its own dependency on `base`. Both paths lead to the definition
    /// in `base`, so there is nothing to choose between, and the association
    /// resolves to it. This is the case that separates a diamond from a clash.
    #[test]
    fn test_association_reachable_by_two_paths_resolves() {
        use weaver_resolved_schema::v2::entity::EntityAssociation;

        let (top, nfes) = resolve_entity_assoc_fixture("top_diamond_assoc");
        assert!(nfes.is_empty(), "unexpected errors: {nfes:?}");

        let [EntityAssociation::Ref(host)] = metric_associations(&top) else {
            panic!(
                "expected one reference, got {:?}",
                metric_associations(&top)
            );
        };
        assert_eq!(ref_source(&top, host), Some(BASE_URL));
    }

    /// An association is not a re-export.
    ///
    /// `middle_assoc_export` names the `host` of `base` and does not import it,
    /// so `host` is no part of what `middle_assoc_export` offers.
    /// `top_transitive_assoc` depends on it alone and cannot reach `host`, as it
    /// could not reach it through an `imports` block either.
    #[test]
    fn test_association_does_not_re_export_the_entity_it_names() {
        let errors = entity_assoc_fixture_fatal_errors("top_transitive_assoc");
        assert!(
            matches!(
                errors.as_slice(),
                [Error::UnresolvedEntityAssociation { entity_type, .. }]
                    if entity_type == "host"
            ),
            "naming an entity does not pass it on: {errors:?}"
        );
    }

    /// Every expression shape resolves, on every signal type that has one, and a
    /// leaf may name an entity refinement as well as an entity type.
    #[test]
    fn test_association_expression_shapes_resolve() {
        use weaver_resolved_schema::v2::entity::EntityAssociation;

        let (schema, nfes) = resolve_entity_assoc_fixture("local_shapes");
        assert!(nfes.is_empty(), "unexpected errors: {nfes:?}");

        // `all_of: [service, one_of: [host, host.windows]]`, where `host` is an
        // entity type and `host.windows` refines it.
        let [EntityAssociation::AllOf { all_of }] = metric_associations(&schema) else {
            panic!(
                "expected one `all_of`, got {:?}",
                metric_associations(&schema)
            );
        };
        let [EntityAssociation::Ref(service), EntityAssociation::OneOf { one_of }] =
            all_of.as_slice()
        else {
            panic!("expected a reference and a `one_of`, got {all_of:?}");
        };
        assert_eq!(&*service.r#type, "service");
        let [EntityAssociation::Ref(host), EntityAssociation::Ref(windows)] = one_of.as_slice()
        else {
            panic!("expected two references, got {one_of:?}");
        };
        assert_eq!(&*host.r#type, "host");
        assert_eq!(
            &*windows.r#type, "host.windows",
            "a leaf may name an entity refinement by its id"
        );
        for entity_ref in [service, host, windows] {
            assert_eq!(ref_source(&schema, entity_ref), None);
        }

        // A span and an event carry associations too, not metrics alone.
        let [span] = schema.registry.spans.as_slice() else {
            panic!("expected one span, got {:?}", schema.registry.spans);
        };
        assert!(
            matches!(
                span.entity_associations.as_slice(),
                [EntityAssociation::OneOf { one_of }] if one_of.len() == 2
            ),
            "a span keeps its expression: {:?}",
            span.entity_associations
        );
        let [event] = schema.registry.events.as_slice() else {
            panic!("expected one event, got {:?}", schema.registry.events);
        };
        assert!(
            matches!(
                event.entity_associations.as_slice(),
                [EntityAssociation::Ref(r)] if &*r.r#type == "service"
            ),
            "an event keeps its expression: {:?}",
            event.entity_associations
        );
    }

    /// A leaf may name an entity refinement that a dependency declares, and the
    /// reference says which registry to read it from.
    #[test]
    fn test_association_names_a_refinement_of_a_dependency() {
        use weaver_resolved_schema::v2::entity::EntityAssociation;

        let (middle, nfes) = resolve_entity_assoc_fixture("middle_refinement_assoc");
        assert!(nfes.is_empty(), "unexpected errors: {nfes:?}");
        assert!(
            entity_types(&middle).is_empty(),
            "an association must not import the entity it names"
        );

        let [EntityAssociation::Ref(windows)] = metric_associations(&middle) else {
            panic!(
                "expected one reference, got {:?}",
                metric_associations(&middle)
            );
        };
        assert_eq!(&*windows.r#type, "host.windows");
        assert_eq!(ref_source(&middle, windows), Some(BASE_URL));
    }

    /// A signal refinement declares associations of its own, and they resolve
    /// against the dependencies as a signal's do.
    #[test]
    fn test_refinement_association_resolves_against_a_dependency() {
        use weaver_resolved_schema::v2::entity::EntityAssociation;

        let (middle, nfes) = resolve_entity_assoc_fixture("middle_refined_signal");
        assert!(nfes.is_empty(), "unexpected errors: {nfes:?}");

        let [refinement] = middle.refinements.metrics.as_slice() else {
            panic!(
                "expected one metric refinement, got {:?}",
                middle.refinements.metrics
            );
        };
        let [EntityAssociation::Ref(host)] = refinement.metric.entity_associations.as_slice()
        else {
            panic!(
                "expected one reference, got {:?}",
                refinement.metric.entity_associations
            );
        };
        assert_eq!(&*host.r#type, "host");
        assert_eq!(ref_source(&middle, host), Some(BASE_URL));
    }

    /// Entity association lookup matches entity definitions only, ignoring
    /// other signals with the same name.
    #[test]
    fn test_association_does_not_match_a_signal_of_another_kind() {
        let errors = entity_assoc_fixture_fatal_errors("middle_name_clash");
        assert!(
            matches!(
                errors.as_slice(),
                [Error::UnresolvedEntityAssociation { entity_type, .. }]
                    if entity_type == "host"
            ),
            "a metric called `host` is no entity: {errors:?}"
        );
    }

    /// A legacy `type: resource` group holds its entity type in `name`, and an
    /// association names the type. The group id is another thing, and an
    /// `imports` pattern that may use it does not make it an entity type.
    #[test]
    fn test_association_to_a_legacy_resource_by_group_id_fails() {
        let errors = entity_assoc_fixture_fatal_errors("legacy_by_id");
        assert!(
            matches!(
                errors.as_slice(),
                [Error::UnresolvedEntityAssociation { entity_type, .. }]
                    if entity_type == "resource.device"
            ),
            "the entity type is `device`, not the group id: {errors:?}"
        );
    }

    /// A public signal must not name an entity its own registry keeps private.
    ///
    /// `dependency_resolution.exclude` keeps the entity out of the surface
    /// offered to dependents, while the metric stays in it. A dependent that
    /// imports the metric gets an association it cannot follow, and the same
    /// name may mean an unrelated entity there. An attribute `ref` and an
    /// `extends` clause both refuse this already.
    #[test]
    fn test_public_signal_associated_with_a_private_entity_is_reported() {
        for target in [ResolutionTarget::V1, ResolutionTarget::V2] {
            let errors =
                entity_assoc_fixture_fatal_errors_with_engine("local_private_assoc", target);
            assert!(
                matches!(
                    errors.as_slice(),
                    [Error::ExcludedFromDependencyResolution { id, used_in, .. }]
                        if id == "host" && used_in == "metric.private.request.count"
                ),
                "a public metric must not export a reference to a private entity: {errors:?}"
            );
        }
    }

    /// A private signal may name a private entity: neither is in the surface
    /// offered to dependents, so nothing dangling leaves the registry. This is
    /// the pass an excluded group gets for an excluded attribute too.
    #[test]
    fn test_private_signal_may_associate_with_a_private_entity() {
        use weaver_resolved_schema::v2::entity::EntityAssociation;

        let (schema, nfes) = resolve_entity_assoc_fixture("local_private_pair");
        assert!(nfes.is_empty(), "unexpected errors: {nfes:?}");

        let [EntityAssociation::Ref(host)] = metric_associations(&schema) else {
            panic!(
                "expected one reference, got {:?}",
                metric_associations(&schema)
            );
        };
        assert_eq!(&*host.r#type, "host");
        assert_eq!(ref_source(&schema, host), None);
    }

    #[test]
    fn test_three_layer_transitive_diamond_upgrade() -> Result<(), Error> {
        // Test that collect_chosen_versions traverses deeply nested multi-layer dependencies.
        let url_base_v1_0 = SchemaUrl::try_from("https://example.com/base/1.0.0").unwrap();
        let url_base_v1_1 = SchemaUrl::try_from("https://example.com/base/1.1.0").unwrap();

        let base_v1_0 = ResolvedDependency::from(ResolvedTelemetrySchema {
            file_format: "resolved/1.0".to_owned(),
            schema_url: "https://example.com/base/1.0.0".to_owned(),
            registry_id: "base".to_owned(),
            registry: weaver_resolved_schema::v1::registry::Registry {
                registry_url: "https://example.com/base/1.0.0".to_owned(),
                entity_association_origins: Default::default(),
                groups: vec![],
            },
            catalog: weaver_resolved_schema::v1::catalog::Catalog::default(),
            resource: None,
            instrumentation_library: None,
            dependencies: Default::default(),
            versions: None,
            registry_manifest: None,
        });

        let base_v1_1 = ResolvedDependency::from(ResolvedTelemetrySchema {
            file_format: "resolved/1.0".to_owned(),
            schema_url: "https://example.com/base/1.1.0".to_owned(),
            registry_id: "base".to_owned(),
            registry: weaver_resolved_schema::v1::registry::Registry {
                registry_url: "https://example.com/base/1.1.0".to_owned(),
                entity_association_origins: Default::default(),
                groups: vec![],
            },
            catalog: weaver_resolved_schema::v1::catalog::Catalog::default(),
            resource: None,
            instrumentation_library: None,
            dependencies: Default::default(),
            versions: None,
            registry_manifest: None,
        });

        let layer1_a = ResolvedDependency::from(ResolvedTelemetrySchema {
            file_format: "resolved/1.0".to_owned(),
            schema_url: "https://example.com/layer1_a/0.1.0".to_owned(),
            registry_id: "layer1_a".to_owned(),
            registry: weaver_resolved_schema::v1::registry::Registry {
                registry_url: "https://example.com/layer1_a/0.1.0".to_owned(),
                entity_association_origins: Default::default(),
                groups: vec![],
            },
            catalog: weaver_resolved_schema::v1::catalog::Catalog::default(),
            resource: None,
            instrumentation_library: None,
            dependencies: [url_base_v1_0.clone()].into_iter().collect(),
            versions: None,
            registry_manifest: None,
        });

        let layer1_b = ResolvedDependency::from(ResolvedTelemetrySchema {
            file_format: "resolved/1.0".to_owned(),
            schema_url: "https://example.com/layer1_b/0.1.0".to_owned(),
            registry_id: "layer1_b".to_owned(),
            registry: weaver_resolved_schema::v1::registry::Registry {
                registry_url: "https://example.com/layer1_b/0.1.0".to_owned(),
                entity_association_origins: Default::default(),
                groups: vec![],
            },
            catalog: weaver_resolved_schema::v1::catalog::Catalog::default(),
            resource: None,
            instrumentation_library: None,
            dependencies: [url_base_v1_1.clone()].into_iter().collect(),
            versions: None,
            registry_manifest: None,
        });

        let mut chosen_versions = HashMap::new();
        layer1_a.collect_chosen_versions(&mut chosen_versions);
        layer1_b.collect_chosen_versions(&mut chosen_versions);
        base_v1_0.collect_chosen_versions(&mut chosen_versions);
        base_v1_1.collect_chosen_versions(&mut chosen_versions);

        assert_eq!(
            chosen_versions.get("example.com/base").unwrap(),
            &url_base_v1_1
        );
        Ok(())
    }

    #[test]
    fn test_chosen_version_upgrade_removed_attribute_fails() -> Result<(), Error> {
        use crate::v1::attribute::{AttributeCatalog, AttributeSource, AttributeWithSource};

        let url_c_v1_1 = SchemaUrl::try_from("https://example.com/c/1.1.0").unwrap();
        let url_c_v1_2 = SchemaUrl::try_from("https://example.com/c/1.2.0").unwrap();

        let mut chosen_versions = HashMap::new();
        let _ = chosen_versions.insert("example.com/c".to_owned(), url_c_v1_2.clone());

        let schema_v1_2 = Arc::new(WeaverResolvedSchema::V1(ResolvedTelemetrySchema {
            file_format: "resolved/1.0".to_owned(),
            schema_url: "https://example.com/c/1.2.0".to_owned(),
            registry_id: "c".to_owned(),
            registry: weaver_resolved_schema::v1::registry::Registry {
                registry_url: "https://example.com/c/1.2.0".to_owned(),
                entity_association_origins: Default::default(),
                groups: vec![],
            },
            catalog: weaver_resolved_schema::v1::catalog::Catalog::default(),
            resource: None,
            instrumentation_library: None,
            dependencies: Default::default(),
            versions: None,
            registry_manifest: None,
        }));

        let mut cache = LruCache::new(NonZeroUsize::new(10).unwrap());
        let _ = cache.push(url_c_v1_2.clone(), schema_v1_2);

        let lookup_ctx = SchemaLookupContext {
            chosen_versions,
            cache: &cache,
            pinned_schemas: HashMap::new(),
        };

        let attr = weaver_resolved_schema::v1::attribute::Attribute {
            name: "c.removed_attr".to_owned(),
            r#type: weaver_semconv::v1::attribute::AttributeType::PrimitiveOrArray(
                weaver_semconv::v1::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            brief: "Old attribute in C v1.1".to_owned(),
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
        };

        let orig_source = AttributeSource::Dependency {
            schema_url: url_c_v1_1.clone(),
        };

        let orig_aws = AttributeWithSource {
            attribute: attr.clone(),
            source: orig_source.clone(),
            is_definition: true,
        };

        let result = AttributeCatalog::upgrade_attribute_with_source(orig_aws, &lookup_ctx);
        match result {
            Err(Error::AttributeNotFoundInUpgradedSchema {
                attribute_name,
                original_url,
                upgraded_url,
            }) => {
                assert_eq!(attribute_name, "c.removed_attr");
                assert_eq!(original_url, url_c_v1_1.to_string());
                assert_eq!(upgraded_url, url_c_v1_2.to_string());
            }
            other => panic!("Expected AttributeNotFoundInUpgradedSchema, got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn test_linearized_dependencies_exclude_superseded_version() -> Result<(), weaver_semconv::Error>
    {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/compatible-version-conflict/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let url_a = SchemaUrl::try_from("https://example.com/a/0.1.0").unwrap();
        let url_b = SchemaUrl::try_from("https://example.com/b/0.1.0").unwrap();
        let url_c_v1_1 = SchemaUrl::try_from("https://example.com/c/1.1.0").unwrap();
        let url_c_v1_2 = SchemaUrl::try_from("https://example.com/c/1.2.0").unwrap();

        assert!(resolved.dependencies.contains(&url_a));
        assert!(resolved.dependencies.contains(&url_b));
        assert!(resolved.dependencies.contains(&url_c_v1_2));
        assert!(
            !resolved.dependencies.contains(&url_c_v1_1),
            "superseded C v1.1 must not appear in linearized dependencies: {:?}",
            resolved.dependencies
        );
        Ok(())
    }

    #[test]
    fn test_linearized_dependencies_exclude_superseded_version_published(
    ) -> Result<(), weaver_semconv::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/compatible-version-conflict-published/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let resolved = match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
            WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r.into_v1().unwrap(),
            WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
        };

        let url_a = SchemaUrl::try_from("https://example.com/a/0.1.0").unwrap();
        let url_b = SchemaUrl::try_from("https://example.com/b/0.1.0").unwrap();
        let url_c_v1_1 = SchemaUrl::try_from("https://example.com/c/1.1.0").unwrap();
        let url_c_v1_2 = SchemaUrl::try_from("https://example.com/c/1.2.0").unwrap();

        assert!(resolved.dependencies.contains(&url_a));
        assert!(resolved.dependencies.contains(&url_b));
        assert!(resolved.dependencies.contains(&url_c_v1_2));
        assert!(
            !resolved.dependencies.contains(&url_c_v1_1),
            "superseded C v1.1 must not appear in linearized dependencies: {:?}",
            resolved.dependencies
        );
        Ok(())
    }

    #[test]
    fn test_resolved_schema_cache_hit_miss_direct_deps() -> Result<(), weaver_semconv::Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/compatible-version-conflict/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(WeaverResolverConfig::default());
        let resolved_bundle =
            match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
                WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
                WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
            };

        let mut cache = match resolver.schema_cache_for(&resolved_bundle) {
            WResult::Ok(c) | WResult::OkWithNFEs(c, _) => c,
            WResult::FatalErr(e) => panic!("building cache snapshot should succeed: {e:?}"),
        };

        let url_a = SchemaUrl::try_from("https://example.com/a/0.1.0").unwrap();
        let url_b = SchemaUrl::try_from("https://example.com/b/0.1.0").unwrap();
        let url_c_v1_2 = SchemaUrl::try_from("https://example.com/c/1.2.0").unwrap();
        let url_c_v1_1 = SchemaUrl::try_from("https://example.com/c/1.1.0").unwrap();

        assert!(matches!(
            cache.resolve_schema(&url_a),
            WResult::Ok(_) | WResult::OkWithNFEs(_, _)
        ));
        assert!(matches!(
            cache.resolve_schema(&url_b),
            WResult::Ok(_) | WResult::OkWithNFEs(_, _)
        ));
        assert!(matches!(
            cache.resolve_schema(&url_c_v1_2),
            WResult::Ok(_) | WResult::OkWithNFEs(_, _)
        ));

        match cache.resolve_schema(&url_c_v1_1) {
            WResult::FatalErr(Error::SchemaNotInCache { schema_url }) => {
                assert_eq!(schema_url, url_c_v1_1);
            }
            WResult::FatalErr(e) => panic!("expected SchemaNotInCache for c/1.1.0, got: {e:?}"),
            _ => panic!("expected FatalErr(SchemaNotInCache) for c/1.1.0"),
        }

        Ok(())
    }

    #[test]
    fn test_snapshot_completeness_under_lru_eviction() -> Result<(), weaver_semconv::Error> {
        let mut overrides = BTreeMap::new();
        _ = overrides.insert(
            SchemaUrl::try_from("https://example.com/a/0.1.0").unwrap(),
            VirtualDirectoryPath::LocalFolder {
                path: "data/compatible-version-conflict/registry_a".to_owned(),
            },
        );
        _ = overrides.insert(
            SchemaUrl::try_from("https://example.com/b/0.1.0").unwrap(),
            VirtualDirectoryPath::LocalFolder {
                path: "data/compatible-version-conflict/registry_b".to_owned(),
            },
        );
        _ = overrides.insert(
            SchemaUrl::try_from("https://example.com/c/1.2.0").unwrap(),
            VirtualDirectoryPath::LocalFolder {
                path: "data/compatible-version-conflict/registry_c_v1_2".to_owned(),
            },
        );

        let config = WeaverResolverConfig {
            cache_capacity: NonZeroUsize::new(1).unwrap(),
            schema_url_overrides: overrides,
            ..Default::default()
        };

        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: "data/compatible-version-conflict/main/registry".to_owned(),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])?;
        let mut resolver = WeaverResolver::new(config);
        let resolved_bundle =
            match resolver.load_and_resolve_schema(registry_repo, DefaultSchemaVisitor) {
                WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
                WResult::FatalErr(e) => panic!("Failed to resolve schema: {e}"),
            };

        let mut cache = match resolver.schema_cache_for(&resolved_bundle) {
            WResult::Ok(c) | WResult::OkWithNFEs(c, _) => c,
            WResult::FatalErr(e) => {
                panic!("building cache snapshot under tiny LRU capacity should succeed: {e:?}")
            }
        };

        let url_a = SchemaUrl::try_from("https://example.com/a/0.1.0").unwrap();
        let url_b = SchemaUrl::try_from("https://example.com/b/0.1.0").unwrap();
        let url_c_v1_2 = SchemaUrl::try_from("https://example.com/c/1.2.0").unwrap();

        assert!(matches!(
            cache.resolve_schema(&url_a),
            WResult::Ok(_) | WResult::OkWithNFEs(_, _)
        ));
        assert!(matches!(
            cache.resolve_schema(&url_b),
            WResult::Ok(_) | WResult::OkWithNFEs(_, _)
        ));
        assert!(matches!(
            cache.resolve_schema(&url_c_v1_2),
            WResult::Ok(_) | WResult::OkWithNFEs(_, _)
        ));

        Ok(())
    }

    #[test]
    fn test_entity_association_origin_upgraded_to_chosen_version() -> Result<(), Error> {
        let url_base_v1_0 = SchemaUrl::try_from("https://example.com/base/1.0.0").unwrap();
        let url_base_v1_1 = SchemaUrl::try_from("https://example.com/base/1.1.0").unwrap();

        let mut chosen_versions = HashMap::new();
        _ = chosen_versions.insert("example.com/base".to_owned(), url_base_v1_1.clone());

        let host_group = weaver_resolved_schema::v1::registry::Group {
            id: "host".to_owned(),
            r#type: GroupType::Entity,
            name: Some("host".to_owned()),
            brief: "Host entity".to_owned(),
            note: Some("".to_owned()),
            prefix: "".to_owned(),
            stability: Default::default(),
            deprecated: Default::default(),
            attributes: vec![],
            extends: None,
            lineage: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            span_kind: None,
            annotations: Default::default(),
            display_name: None,
            body: None,
            entity_associations: vec![],
            visibility: None,
            is_v2: false,
            span_name: None,
            span_links: Vec::new(),
            requirement_level: Default::default(),
        };

        let schema_base_v1_1 = Arc::new(WeaverResolvedSchema::V1(ResolvedTelemetrySchema {
            file_format: "resolved/1.0".to_owned(),
            schema_url: "https://example.com/base/1.1.0".to_owned(),
            registry_id: "base".to_owned(),
            registry: weaver_resolved_schema::v1::registry::Registry {
                registry_url: "https://example.com/base/1.1.0".to_owned(),
                entity_association_origins: Default::default(),
                groups: vec![host_group],
            },
            catalog: weaver_resolved_schema::v1::catalog::Catalog::default(),
            resource: None,
            instrumentation_library: None,
            dependencies: Default::default(),
            versions: None,
            registry_manifest: None,
        }));

        let mut cache = LruCache::new(NonZeroUsize::new(10).unwrap());
        let _ = cache.push(url_base_v1_1.clone(), schema_base_v1_1);

        let lookup_ctx = SchemaLookupContext {
            chosen_versions,
            cache: &cache,
            pinned_schemas: HashMap::new(),
        };

        let upgraded = v1::upgrade::upgrade_entity_origin("host", &url_base_v1_0, &lookup_ctx)?;
        assert_eq!(upgraded, url_base_v1_1);

        let missing =
            v1::upgrade::upgrade_entity_origin("missing_entity", &url_base_v1_0, &lookup_ctx);
        match missing {
            Err(Error::EntityNotFoundInUpgradedSchema {
                entity_name,
                original_url,
                upgraded_url,
            }) => {
                assert_eq!(entity_name, "missing_entity");
                assert_eq!(original_url, url_base_v1_0.to_string());
                assert_eq!(upgraded_url, url_base_v1_1.to_string());
            }
            other => panic!("Expected EntityNotFoundInUpgradedSchema, got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn test_pure_v2_fixtures_never_enter_v1_code() {
        let pure_v2_fixtures = [
            "data/registry-test-v2-1-everything",
            "data/registry-test-v2-2-multifile",
            "data/registry-test-v2-4-refinements",
            "data/registry-test-v2-5-published",
            "data/registry-test-v2-dep/app_registry",
            "data/registry-test-v2-dep/bare_ref_registry",
            "data/registry-test-v2-dep/consumer_published",
            "data/registry-test-v2-dep/consumer_registry",
            "data/registry-test-v2-dep/deep_registry",
            "data/registry-test-v2-dep/entity_registry",
            "data/registry-test-v2-dep/event_registry",
            "data/registry-test-v2-dep/group_import_registry",
            "data/registry-test-v2-dep/metric_registry",
            "data/registry-test-v2-dep/signal_import_registry",
            "data/registry-test-v2-dep/span_import_registry",
            "data/registry-test-v2-dep/span_links_registry",
            "data/registry-test-v2-dep/span_registry",
            "data/entity-assoc-imports/local_shapes",
            "data/entity-assoc-imports/local_private_pair",
            "data/entity-assoc-imports/middle",
            "data/entity-assoc-imports/middle_reexport",
            "data/entity-assoc-imports/middle_refined_signal",
            "data/entity-assoc-imports/middle_refinement_assoc",
            "data/entity-assoc-imports/top_diamond",
            "data/entity-assoc-imports/top_diamond_assoc",
            "data/entity-assoc-imports/top_private_first",
            "data/entity-assoc-imports/top_public_first",
            "data/entity-assoc-imports/top_rebind",
            "data/entity-assoc-imports/top_rival_import",
            "data/inherited-ref-test/base",
            "data/inherited-ref-test/middle",
            "data/inherited-ref-test/refiner",
            "data/signal-name-preservation/v2",
            "data/registry-test-dep-exclusion/migration_parent",
            "data/registry-test-dep-exclusion/migration_consumer",
        ];

        assert!(
            pure_v2_fixtures.len() >= 25,
            "Expected at least 25 pure-V2 fixtures"
        );

        let _engine_guard = DisallowV1EngineScope::new();
        let _adapter_guard = DisallowV1AdapterScope::new();

        for fixture in pure_v2_fixtures {
            let registry_path = if std::path::Path::new(&format!("{fixture}/registry")).is_dir() {
                format!("{fixture}/registry")
            } else {
                fixture.to_owned()
            };

            let vpath = VirtualDirectoryPath::LocalFolder {
                path: registry_path,
            };
            let mut repo_nfes = Vec::new();
            let repo = RegistryRepo::try_new(None, &vpath, &mut repo_nfes)
                .unwrap_or_else(|e| panic!("Failed to load repo for `{fixture}`: {e}"));

            let config = WeaverResolverConfig {
                target: ResolutionTarget::V2,
                ..Default::default()
            };
            let mut resolver = WeaverResolver::new(config);
            match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
                WResult::Ok(schema) | WResult::OkWithNFEs(schema, _) => {
                    assert!(
                        schema.as_v2().is_some(),
                        "Expected V2 schema for pure-V2 fixture `{fixture}`"
                    );
                }
                WResult::FatalErr(e) => {
                    assert_eq!(
                        fixture, "data/entity-assoc-imports/top_rival_import",
                        "Pure V2 fixture `{fixture}` failed to resolve: {e}"
                    );
                }
            }
        }
    }

    fn resolve_without_identity_fixture(
        name: &str,
        target: ResolutionTarget,
    ) -> WResult<WeaverResolvedSchema, Error> {
        let registry_path = VirtualDirectoryPath::LocalFolder {
            path: format!("data/v2-entity-without-identity/{name}"),
        };
        let registry_repo = RegistryRepo::try_new(None, &registry_path, &mut vec![])
            .expect("Failed to load the registry");
        WeaverResolver::new(WeaverResolverConfig::default().with_target(target))
            .load_and_resolve_schema(registry_repo, DefaultSchemaVisitor)
            .ignore(is_unstable_format_warning)
    }

    fn is_converted_warning(nfes: &[Error]) -> bool {
        matches!(nfes, [Error::EntityConvertedToAttributeGroup { group_id, .. }] if group_id == "entity.legacy")
    }

    /// Resolved for V2, a V1 entity without identity becomes a public attribute
    /// group, with a warning. Resolved for V1, it stays an entity.
    #[test]
    fn test_entity_without_identity_becomes_attribute_group() {
        let (v1, nfes) = resolve_without_identity_fixture("local", ResolutionTarget::V1)
            .into_result_with_non_fatal()
            .expect("Failed to resolve");
        assert_eq!(
            v1.as_v1().unwrap().group("entity.legacy").unwrap().r#type,
            GroupType::Entity
        );
        assert!(nfes.is_empty(), "unexpected warnings: {nfes:?}");

        let (v2, nfes) = resolve_without_identity_fixture("local", ResolutionTarget::V2)
            .into_result_with_non_fatal()
            .expect("Failed to resolve");
        let registry = &v2.as_v2().unwrap().registry;
        assert!(registry.entities.is_empty());
        assert!(registry
            .attribute_groups
            .iter()
            .any(|g| &*g.id == "entity.legacy"));
        assert!(is_converted_warning(&nfes), "unexpected warnings: {nfes:?}");
    }

    /// Once converted, the entity can't be associated with, whether it is
    /// defined locally or in a dependency.
    #[test]
    fn test_entity_without_identity_cannot_be_associated() {
        for name in ["local_assoc", "consumer"] {
            assert!(
                resolve_without_identity_fixture(name, ResolutionTarget::V1)
                    .into_result_failing_non_fatal()
                    .is_ok(),
                "{name} must resolve for V1"
            );
            let error = resolve_without_identity_fixture(name, ResolutionTarget::V2)
                .into_result_with_non_fatal()
                .expect_err("association to a converted entity must fail");
            // A local association fails when converting to V2, a dependency one during resolution.
            let message = format!("{error:?}");
            assert!(
                (message.contains("UnresolvedEntityAssociation")
                    || message.contains("is associated with entity legacy"))
                    && message.contains("legacy"),
                "{name}: {message}"
            );
        }
    }

    /// Refining a converted entity fails, whether resolved from source or from a pre-resolved
    /// registry.
    #[test]
    fn test_entity_without_identity_cannot_be_refined() {
        let v1 = resolve_without_identity_fixture("refinement", ResolutionTarget::V1)
            .into_result_failing_non_fatal()
            .expect("Failed to resolve")
            .as_v1()
            .unwrap()
            .clone();
        let mut resolver =
            WeaverResolver::new(WeaverResolverConfig::default().with_target(ResolutionTarget::V2));
        let pre_resolved = resolver
            .resolve_loaded(LoadedSemconvRegistry::Resolved {
                schema: v1,
                direct_dependencies: vec![],
            })
            .into_result_with_non_fatal()
            .map(|_| ());
        let from_source = resolve_without_identity_fixture("refinement", ResolutionTarget::V2)
            .into_result_with_non_fatal()
            .map(|_| ());
        for result in [pre_resolved, from_source] {
            let message = result
                .expect_err("refining a converted entity must fail")
                .to_string();
            assert!(message.contains("entity.legacy.child"), "{message}");
        }
    }

    /// A converted entity takes its group id in the attribute group namespace,
    /// so it can't share that id with an existing public attribute group.
    #[test]
    fn test_entity_without_identity_collides_with_attribute_group() {
        let (_, nfes) = resolve_without_identity_fixture("collision", ResolutionTarget::V2)
            .into_result_with_non_fatal()
            .expect("Failed to resolve");
        assert!(
            nfes.iter().any(|e| matches!(e, Error::CollidingV2SignalId { signal_id, .. } if signal_id == "entity.legacy")),
            "unexpected errors: {nfes:?}"
        );
    }

    /// A V1 registry that was resolved before loading converts the same way.
    #[test]
    fn test_entity_without_identity_in_pre_resolved_registry() {
        let schema = resolve_without_identity_fixture("local", ResolutionTarget::V1)
            .into_result_failing_non_fatal()
            .expect("Failed to resolve")
            .as_v1()
            .unwrap()
            .clone();
        let mut resolver =
            WeaverResolver::new(WeaverResolverConfig::default().with_target(ResolutionTarget::V2));
        let (resolved, nfes) = resolver
            .resolve_loaded(LoadedSemconvRegistry::Resolved {
                schema,
                direct_dependencies: vec![],
            })
            .into_result_with_non_fatal()
            .expect("Failed to load");
        assert!(resolved.as_v2().unwrap().registry.entities.is_empty());
        assert!(is_converted_warning(&nfes), "unexpected warnings: {nfes:?}");
    }
}
