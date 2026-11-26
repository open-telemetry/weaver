// SPDX-License-Identifier: Apache-2.0

//! Unit tests for checked dependency version upgrades.

use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use weaver_resolved_schema::v1::attribute::Attribute;
use weaver_resolved_schema::v1::registry::{Group, Registry};
use weaver_resolved_schema::v1::ResolvedTelemetrySchema;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v1::attribute::{AttributeType, PrimitiveOrArrayTypeSpec};
use weaver_semconv::v1::group::GroupType;

use crate::error::Error;
use crate::v1::attribute::{AttributeSource, AttributeWithSource};
use crate::v1::dependency::{GroupSource, GroupSummary};
use crate::v1::upgrade::{upgrade_attribute, upgrade_entity_origin, upgrade_refinement_parent};
use crate::{SchemaLookupContext, WeaverResolvedSchema};

fn make_test_context(
    chosen_url: SchemaUrl,
    schema: WeaverResolvedSchema,
) -> (
    LruCache<SchemaUrl, Arc<WeaverResolvedSchema>>,
    HashMap<String, SchemaUrl>,
) {
    let mut chosen_versions = HashMap::new();
    _ = chosen_versions.insert(chosen_url.name().to_owned(), chosen_url.clone());
    let mut cache = LruCache::new(NonZeroUsize::new(10).unwrap());
    _ = cache.push(chosen_url, Arc::new(schema));
    (cache, chosen_versions)
}

#[test]
fn test_attribute_upgrade_success_and_failure() {
    let url_v1_0 = SchemaUrl::try_from("https://example.com/dep/1.0.0").unwrap();
    let url_v1_1 = SchemaUrl::try_from("https://example.com/dep/1.1.0").unwrap();

    let upgraded_attr = Attribute {
        name: "test.attr".to_owned(),
        r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
        brief: "Upgraded brief in 1.1.0".to_owned(),
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

    let mut root_attributes = HashMap::new();
    _ = root_attributes.insert(
        "test.attr".to_owned(),
        weaver_resolved_schema::v1::catalog::RootAttribute {
            attribute: upgraded_attr.clone(),
            source_group: "test".to_owned(),
            is_definition: true,
        },
    );
    let catalog =
        weaver_resolved_schema::v1::catalog::Catalog::new(vec![upgraded_attr], root_attributes);

    let schema_v1_1 = WeaverResolvedSchema::V1(ResolvedTelemetrySchema {
        file_format: "resolved/1.0".to_owned(),
        schema_url: "https://example.com/dep/1.1.0".to_owned(),
        registry_id: "dep".to_owned(),
        registry: Registry {
            registry_url: "https://example.com/dep/1.1.0".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![],
        },
        catalog,
        resource: None,
        instrumentation_library: None,
        dependencies: Default::default(),
        versions: None,
        registry_manifest: None,
    });

    let (cache, chosen_versions) = make_test_context(url_v1_1.clone(), schema_v1_1);
    let lookup_ctx = SchemaLookupContext {
        chosen_versions,
        cache: &cache,
        pinned_schemas: HashMap::new(),
    };

    let orig_attr = Attribute {
        name: "test.attr".to_owned(),
        r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
        brief: "Old brief in 1.0.0".to_owned(),
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

    let aws = AttributeWithSource {
        attribute: orig_attr,
        source: AttributeSource::Dependency {
            schema_url: url_v1_0.clone(),
        },
        is_definition: true,
    };

    let upgraded = upgrade_attribute(aws, &lookup_ctx).expect("upgrade should succeed");
    assert_eq!(upgraded.attribute.brief, "Upgraded brief in 1.1.0");
    match upgraded.source {
        AttributeSource::Dependency { schema_url } => assert_eq!(schema_url, url_v1_1),
        AttributeSource::Local { .. } => panic!("expected dependency source"),
    }

    let missing_aws = AttributeWithSource {
        attribute: Attribute {
            name: "missing.attr".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: "Old".to_owned(),
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
        },
        source: AttributeSource::Dependency {
            schema_url: url_v1_0.clone(),
        },
        is_definition: true,
    };

    match upgrade_attribute(missing_aws, &lookup_ctx) {
        Err(Error::AttributeNotFoundInUpgradedSchema {
            attribute_name,
            original_url,
            upgraded_url,
        }) => {
            assert_eq!(attribute_name, "missing.attr");
            assert_eq!(original_url, url_v1_0.to_string());
            assert_eq!(upgraded_url, url_v1_1.to_string());
        }
        other => panic!("expected AttributeNotFoundInUpgradedSchema, got {other:?}"),
    }
}

#[test]
fn test_refinement_parent_upgrade() {
    let url_v1_0 = SchemaUrl::try_from("https://example.com/dep/1.0.0").unwrap();
    let url_v1_1 = SchemaUrl::try_from("https://example.com/dep/1.1.0").unwrap();

    let upgraded_parent_group = Group {
        id: "http.client".to_owned(),
        r#type: GroupType::Span,
        name: None,
        brief: "Upgraded HTTP client span".to_owned(),
        note: None,
        prefix: "http".to_owned(),
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

    let schema_v1_1 = WeaverResolvedSchema::V1(ResolvedTelemetrySchema {
        file_format: "resolved/1.0".to_owned(),
        schema_url: "https://example.com/dep/1.1.0".to_owned(),
        registry_id: "dep".to_owned(),
        registry: Registry {
            registry_url: "https://example.com/dep/1.1.0".to_owned(),
            entity_association_origins: Default::default(),
            groups: vec![upgraded_parent_group.clone()],
        },
        catalog: Default::default(),
        resource: None,
        instrumentation_library: None,
        dependencies: Default::default(),
        versions: None,
        registry_manifest: None,
    });

    let (cache, chosen_versions) = make_test_context(url_v1_1.clone(), schema_v1_1);
    let lookup_ctx = SchemaLookupContext {
        chosen_versions,
        cache: &cache,
        pinned_schemas: HashMap::new(),
    };

    let mut parent_summary =
        GroupSummary::from_without_attributes(&upgraded_parent_group, GroupSource::Dependency);
    parent_summary.origin = Some(url_v1_0.clone());

    let upgraded_summary = upgrade_refinement_parent(
        parent_summary.clone(),
        "http.client",
        "my_refinement",
        &lookup_ctx,
    )
    .expect("parent upgrade should succeed");
    assert_eq!(upgraded_summary.brief, "Upgraded HTTP client span");

    match upgrade_refinement_parent(
        parent_summary,
        "missing.group",
        "my_refinement",
        &lookup_ctx,
    ) {
        Err(Error::GroupNotFoundInUpgradedSchema {
            group_id,
            original_url,
            upgraded_url,
        }) => {
            assert_eq!(group_id, "missing.group");
            assert_eq!(original_url, url_v1_0.to_string());
            assert_eq!(upgraded_url, url_v1_1.to_string());
        }
        other => panic!("expected GroupNotFoundInUpgradedSchema, got {other:?}"),
    }
}

#[test]
fn test_chosen_schema_not_in_cache_returns_error() {
    let url_v1_0 = SchemaUrl::try_from("https://example.com/dep/1.0.0").unwrap();
    let url_v1_1 = SchemaUrl::try_from("https://example.com/dep/1.1.0").unwrap();

    let mut chosen_versions = HashMap::new();
    _ = chosen_versions.insert(url_v1_1.name().to_owned(), url_v1_1.clone());

    let cache = LruCache::new(NonZeroUsize::new(10).unwrap());
    let lookup_ctx = SchemaLookupContext {
        chosen_versions,
        cache: &cache,
        pinned_schemas: HashMap::new(),
    };

    let result = upgrade_entity_origin("host", &url_v1_0, &lookup_ctx);
    match result {
        Err(Error::ChosenSchemaNotInCache { schema_url }) => {
            assert_eq!(schema_url, url_v1_1);
        }
        other => panic!("expected ChosenSchemaNotInCache, got {other:?}"),
    }
}
