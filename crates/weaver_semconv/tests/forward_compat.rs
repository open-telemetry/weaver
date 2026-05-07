// SPDX-License-Identifier: Apache-2.0

//! Integration tests: fixtures that plant unknown fields load without fatal error,
//! and `collect_paths` (the same diff the loader uses) reports each by dotted path.
//! For `definition/2` (no minor) and definition manifests, unknowns are log-only.

use weaver_common::result::WResult;
use weaver_semconv::manifest::{DefinitionRegistryManifest, RegistryManifest};
use weaver_semconv::semconv::SemConvSpecWithProvenance;
use weaver_semconv::v2::SemConvSpecV2;
use weaver_semconv::Error;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("test_data")
        .join(name)
}

fn strip_format_keys(value: serde_yaml::Value) -> serde_yaml::Value {
    match value {
        serde_yaml::Value::Mapping(mut m) => {
            let _ = m.remove(serde_yaml::Value::String("file_format".to_owned()));
            let _ = m.remove(serde_yaml::Value::String("version".to_owned()));
            serde_yaml::Value::Mapping(m)
        }
        other => other,
    }
}

#[test]
fn definition_v2_tolerates_and_warns_on_unknown_fields_throughout() {
    let path = fixture("definition_v2_with_typos.yaml");

    // Loader path: file must load. Reporting is log-only; we re-derive the field list below.
    let result = SemConvSpecWithProvenance::from_file(
        weaver_semconv::schema_url::SchemaUrl::new_unknown(),
        &path,
    );
    match result {
        WResult::Ok(_) | WResult::OkWithNFEs(_, _) => {}
        WResult::FatalErr(e) => panic!("expected the file to load, got fatal: {e:?}"),
    }

    let raw_yaml = std::fs::read_to_string(&path).expect("read fixture");
    let raw: serde_yaml::Value =
        serde_yaml::from_str(&raw_yaml).expect("fixture should parse as YAML");
    let cleaned = strip_format_keys(raw);
    let typed: SemConvSpecV2 = serde_yaml::from_value(cleaned.clone())
        .expect("fixture should deserialize as SemConvSpecV2");
    let unknowns = weaver_semconv::unexpected_fields::collect_paths(&cleaned, &typed);

    let expected = [
        "typo_top_level",
        "attributes[0].typo_in_attribute",
        "attributes[0].deprecated.typo_in_deprecated",
        "attributes[1].type.typo_in_attribute_type_enum",
        "attributes[1].type.members[0].typo_in_enum_member",
        "attribute_groups[0].typo_in_public_attr_group",
        "metrics[0].typo_in_metric",
        "spans[0].typo_in_span",
        "spans[0].name.typo_in_span_name",
        "spans[0].attributes[0].typo_in_span_attribute_ref",
        "spans[0].attributes[0].requirement_level.typo_in_req_level",
        "events[0].typo_in_event",
        "entities[0].typo_in_entity",
        "imports.typo_in_imports",
    ];
    for p in expected {
        assert!(
            unknowns.iter().any(|f| f == p),
            "expected unknown-field path {p:?} to be reported, got: {unknowns:?}"
        );
    }
}

/// Mirror of the v2-semconv test for `DefinitionRegistryManifest`. No `file_format`,
/// so unknowns are log-only; the manifest still loads.
#[test]
fn definition_manifest_tolerates_and_warns_on_unknown_fields() {
    let path = fixture("definition_manifest_with_typos.yaml");

    let mut nfes: Vec<Error> = vec![];
    let manifest =
        RegistryManifest::try_from_file(&path, &mut nfes).expect("definition manifest should load");
    assert!(
        matches!(manifest, RegistryManifest::Definition(_)),
        "expected Definition variant, got {manifest:?}"
    );

    let raw_yaml = std::fs::read_to_string(&path).expect("read fixture");
    let raw: serde_yaml::Value =
        serde_yaml::from_str(&raw_yaml).expect("fixture should parse as YAML");
    let cleaned = strip_format_keys(raw);
    let def: DefinitionRegistryManifest = serde_yaml::from_value(cleaned.clone())
        .expect("fixture should deserialize as DefinitionRegistryManifest");
    let unknowns = weaver_semconv::unexpected_fields::collect_paths(&cleaned, &def);

    let expected = ["typo_top_level", "dependencies[0].typo_in_dependency"];
    for p in expected {
        assert!(
            unknowns.iter().any(|f| f == p),
            "expected unknown-field path {p:?} to be reported, got: {unknowns:?}"
        );
    }
}
