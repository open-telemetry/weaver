// SPDX-License-Identifier: Apache-2.0

//! Golden-file integration tests comparing resolved V2 schemas (`expected-schema.yaml`),
//! warnings (`expected-warnings.json`), and errors (`expected-errors.json`) under `data/`.

mod common;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use weaver_common::result::WResult;
use weaver_common::vdir::VirtualDirectoryPath;
use weaver_resolved_schema::v2::ResolvedTelemetrySchema;
use weaver_resolver::{
    DefaultSchemaVisitor, Error, ResolutionTarget, WeaverResolvedSchema, WeaverResolver,
    WeaverResolverConfig,
};
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v1::attribute::AttributeRole;
use weaver_semconv::Error as SemconvError;

use common::{
    assert_resolved_errors_eq, assert_resolved_v2_schema_eq, assert_resolved_warnings_eq,
};

const EXPECTED_SCHEMA_FILE: &str = "expected-schema.yaml";
const EXPECTED_ERRORS_FILE: &str = "expected-errors.json";
const EXPECTED_WARNINGS_FILE: &str = "expected-warnings.json";
const SETTINGS_FILE: &str = "settings.yaml";

/// Settings loaded from an optional `settings.yaml` inside a test directory.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct TestSettings {
    /// Optional fixed schema URL to pass when creating the RegistryRepo.
    pub schema_url: Option<String>,
    /// Optional schema URL overrides mapping schema URL to local directory path.
    #[serde(default)]
    pub schema_url_overrides: HashMap<String, String>,
}

/// Checks if a non-fatal warning is one of the allowed known warnings.
fn is_allowed_warning(test_dir: &str, err: &Error) -> bool {
    match err {
        Error::FailToResolveDefinition(semconv_err) => match semconv_err {
            SemconvError::UnstableFileFormat { .. }
            | SemconvError::LegacyRegistryManifest { .. }
            | SemconvError::DeprecatedVersionField { .. }
            | SemconvError::MissingManifestFileFormat { .. } => true,
            SemconvError::InvalidGroupUsesPrefix { .. } => test_dir.contains("prefix"),
            _ => false,
        },
        _ => false,
    }
}

/// Checks if a semconv non-fatal warning from `RegistryRepo::try_new` is allowed.
fn is_allowed_semconv_warning(test_dir: &str, err: &SemconvError) -> bool {
    match err {
        SemconvError::UnstableFileFormat { .. }
        | SemconvError::LegacyRegistryManifest { .. }
        | SemconvError::DeprecatedVersionField { .. }
        | SemconvError::MissingManifestFileFormat { .. } => true,
        SemconvError::InvalidGroupUsesPrefix { .. } => test_dir.contains("prefix"),
        _ => false,
    }
}

/// Expected outcome kind for a discovered golden test fixture.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum TestCaseKind {
    Schema,
    Errors,
}

/// A discovered test case under `data/`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TestCase {
    test_dir: String,
    kind: TestCaseKind,
}

/// Discovers all test cases under `data/` containing `expected-schema.yaml` or `expected-errors.json`.
fn discover_test_cases() -> Vec<TestCase> {
    let mut cases = Vec::new();
    for entry in WalkDir::new("data").into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let file_name = entry.file_name().to_string_lossy();
            if file_name == EXPECTED_SCHEMA_FILE {
                if let Some(parent) = entry.path().parent() {
                    cases.push(TestCase {
                        test_dir: parent.display().to_string().replace('\\', "/"),
                        kind: TestCaseKind::Schema,
                    });
                }
            } else if file_name == EXPECTED_ERRORS_FILE {
                if let Some(parent) = entry.path().parent() {
                    cases.push(TestCase {
                        test_dir: parent.display().to_string().replace('\\', "/"),
                        kind: TestCaseKind::Errors,
                    });
                }
            }
        }
    }
    cases.sort();
    cases
}

/// Loads settings from `settings.yaml` if present in `test_dir`.
fn load_settings(test_dir: &str) -> TestSettings {
    let path = Path::new(test_dir).join(SETTINGS_FILE);
    if path.exists() {
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));
        serde_saphyr::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", path.display()))
    } else {
        TestSettings::default()
    }
}

/// Resolves a fixture and returns the result (or fatal error) and observed unignored warnings.
fn resolve_test_case(
    test_dir: &str,
    settings: &TestSettings,
    target: ResolutionTarget,
) -> (Result<ResolvedTelemetrySchema, Error>, Vec<Error>) {
    let registry_subfolder = Path::new(test_dir).join("registry");
    assert!(
        registry_subfolder.is_dir(),
        "Registry directory `{}` does not exist",
        registry_subfolder.display()
    );
    let target_path = registry_subfolder.display().to_string().replace('\\', "/");

    let schema_url: Option<SchemaUrl> = settings
        .schema_url
        .as_deref()
        .map(|s| s.try_into().expect("valid schema_url in settings"))
        .or_else(|| {
            Some(
                "https://default/0.1.0"
                    .try_into()
                    .expect("valid default schema url"),
            )
        });

    let location = VirtualDirectoryPath::LocalFolder { path: target_path };
    let mut repo_nfes = Vec::new();
    let repo = match RegistryRepo::try_new(schema_url, &location, &mut repo_nfes) {
        Ok(r) => r,
        Err(e) => {
            return (Err(Error::FailToResolveDefinition(e)), Vec::new());
        }
    };

    let mut observed_warnings = Vec::new();
    for nfe in repo_nfes {
        if !is_allowed_semconv_warning(test_dir, &nfe) {
            observed_warnings.push(Error::FailToResolveDefinition(nfe));
        }
    }

    let mut resolver = WeaverResolver::new(WeaverResolverConfig::default().with_target(target));
    for (url_str, local_path) in &settings.schema_url_overrides {
        let url: SchemaUrl = url_str
            .as_str()
            .try_into()
            .expect("valid override schema_url");
        resolver.add_schema_url_override(
            url,
            VirtualDirectoryPath::LocalFolder {
                path: local_path.clone(),
            },
        );
    }
    let resolved = match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
        WResult::Ok(r) => Ok(r),
        WResult::OkWithNFEs(r, nfes) => {
            for nfe in nfes {
                if !is_allowed_warning(test_dir, &nfe) {
                    observed_warnings.push(nfe);
                }
            }
            Ok(r)
        }
        WResult::FatalErr(e) => Err(e),
    };

    let schema_res = match resolved {
        Ok(WeaverResolvedSchema::V2(v2)) => Ok(v2),
        Ok(WeaverResolvedSchema::V1(v1)) => match ResolvedTelemetrySchema::try_from(v1) {
            Ok(v2) => Ok(v2),
            Err(e) => panic!("V1 to V2 schema conversion failed for `{test_dir}`: {e}"),
        },
        Err(e) => Err(e),
    };

    (schema_res, observed_warnings)
}

#[test]
fn test_v2_goldens() {
    let cases = discover_test_cases();
    assert!(!cases.is_empty(), "No test cases discovered under data/");

    for case in &cases {
        let settings = load_settings(&case.test_dir);
        let (res, warnings) = resolve_test_case(&case.test_dir, &settings, ResolutionTarget::V1);

        match case.kind {
            TestCaseKind::Schema => {
                // Check warnings against expected-warnings.json if present
                let expected_warnings_file = Path::new(&case.test_dir).join(EXPECTED_WARNINGS_FILE);
                if expected_warnings_file.exists() {
                    let expected_warnings_json = fs::read_to_string(&expected_warnings_file)
                        .unwrap_or_else(|e| {
                            panic!("Failed to read {}: {e}", expected_warnings_file.display())
                        });
                    let observed_warnings_json = serde_json::to_string_pretty(&warnings)
                        .unwrap_or_else(|e| panic!("Failed to serialize warnings: {e}"));
                    assert_resolved_warnings_eq(
                        &case.test_dir,
                        &expected_warnings_json,
                        &observed_warnings_json,
                    );
                } else {
                    assert!(
                        warnings.is_empty(),
                        "Unexpected non-fatal warnings in `{}`: {warnings:?}",
                        case.test_dir,
                    );
                }

                let observed_schema = res.unwrap_or_else(|e| {
                    panic!(
                        "Resolution failed unexpectedly for `{}`: {e:?}",
                        case.test_dir
                    )
                });

                // Write observed output to observed_output/<test_dir>/schema.yaml
                let observed_dir = Path::new("observed_output").join(&case.test_dir);
                fs::create_dir_all(&observed_dir)
                    .unwrap_or_else(|e| panic!("Failed to create observed output dir: {e}"));
                let observed_yaml = serde_saphyr::to_string(&observed_schema)
                    .unwrap_or_else(|e| panic!("Failed to serialize observed schema: {e}"));
                fs::write(observed_dir.join("schema.yaml"), &observed_yaml)
                    .unwrap_or_else(|e| panic!("Failed to write observed schema: {e}"));

                let expected_file = Path::new(&case.test_dir).join(EXPECTED_SCHEMA_FILE);
                let expected_yaml = fs::read_to_string(&expected_file)
                    .unwrap_or_else(|e| panic!("Failed to read {}: {e}", expected_file.display()));
                let expected_schema: ResolvedTelemetrySchema =
                    serde_saphyr::from_str(&expected_yaml).unwrap_or_else(|e| {
                        panic!("Failed to deserialize {}: {e}", expected_file.display())
                    });

                assert_resolved_v2_schema_eq(
                    &case.test_dir,
                    &expected_schema,
                    &observed_schema,
                    &observed_yaml,
                );
            }
            TestCaseKind::Errors => {
                let observed_err = match res {
                    Err(e) => e,
                    Ok(_) => {
                        if warnings.is_empty() {
                            panic!(
                                "Test `{}` was expected to fail with errors but succeeded with no warnings",
                                case.test_dir
                            );
                        } else if warnings.len() == 1 {
                            warnings.into_iter().next().unwrap()
                        } else {
                            Error::CompoundError(warnings)
                        }
                    }
                };
                let observed_json = serde_json::to_string(&observed_err)
                    .unwrap_or_else(|e| panic!("Failed to serialize error: {e}"));
                let expected_file = Path::new(&case.test_dir).join(EXPECTED_ERRORS_FILE);
                let expected_json = fs::read_to_string(&expected_file)
                    .unwrap_or_else(|e| panic!("Failed to read {}: {e}", expected_file.display()));

                assert_resolved_errors_eq(&case.test_dir, &expected_json, &observed_json);
            }
        }
    }
}

#[test]
fn test_v2_goldens_native_engine() {
    let cases = discover_test_cases();
    assert!(!cases.is_empty(), "No test cases discovered under data/");

    for case in &cases {
        let settings = load_settings(&case.test_dir);
        let (res, raw_warnings) =
            resolve_test_case(&case.test_dir, &settings, ResolutionTarget::V2);

        let has_mixed_warning = raw_warnings
            .iter()
            .any(|w| matches!(w, Error::MixedRegistryUsesV1Path { .. }));

        if case.kind == TestCaseKind::Schema {
            let is_mixed_fixture = matches!(
                case.test_dir.as_str(),
                "data/registry-test-v2-3-both-version"
                    | "data/entity-identity-dependency-role/consumer"
                    | "data/entity-identity-inheritance"
            );

            if is_mixed_fixture {
                assert!(
                    has_mixed_warning,
                    "Expected MixedRegistryUsesV1Path warning for mixed fixture `{}`",
                    case.test_dir
                );
            } else {
                assert!(
                    !has_mixed_warning,
                    "Unexpected MixedRegistryUsesV1Path warning for pure fixture `{}`",
                    case.test_dir
                );
            }
        }

        let warnings: Vec<Error> = raw_warnings
            .into_iter()
            .filter(|w| {
                !matches!(
                    w,
                    Error::MixedRegistryUsesV1Path { .. } | Error::V1DependencyUsesV1Path { .. }
                )
            })
            .collect();

        match case.kind {
            TestCaseKind::Schema => {
                // Native V2 treats rival imported definitions as fatal errors rather than warnings.
                if case.test_dir == "data/entity-assoc-imports/top_rival_import" {
                    let err = res
                        .expect_err("Expected fatal error for rival import collision in native V2");
                    let msg = err.to_string();
                    assert!(
                        msg.contains("entity.host") && msg.contains("host"),
                        "Expected duplicate entity.host / host error, got: {msg}"
                    );
                    continue;
                }

                // Check warnings against expected-warnings.json if present
                let expected_warnings_file = Path::new(&case.test_dir).join(EXPECTED_WARNINGS_FILE);
                if expected_warnings_file.exists() {
                    let expected_warnings_json = fs::read_to_string(&expected_warnings_file)
                        .unwrap_or_else(|e| {
                            panic!("Failed to read {}: {e}", expected_warnings_file.display())
                        });
                    let observed_warnings_json = serde_json::to_string_pretty(&warnings)
                        .unwrap_or_else(|e| panic!("Failed to serialize warnings: {e}"));
                    assert_resolved_warnings_eq(
                        &case.test_dir,
                        &expected_warnings_json,
                        &observed_warnings_json,
                    );
                } else {
                    assert!(
                        warnings.is_empty(),
                        "Unexpected non-fatal warnings in `{}`: {warnings:?}",
                        case.test_dir,
                    );
                }

                let observed_schema = res.unwrap_or_else(|e| {
                    panic!(
                        "Resolution failed unexpectedly for `{}`: {e:?}",
                        case.test_dir
                    )
                });

                // Write observed output to observed_output_native/<test_dir>/schema.yaml
                let observed_dir = Path::new("observed_output_native").join(&case.test_dir);
                fs::create_dir_all(&observed_dir)
                    .unwrap_or_else(|e| panic!("Failed to create observed output dir: {e}"));
                let observed_yaml = serde_saphyr::to_string(&observed_schema)
                    .unwrap_or_else(|e| panic!("Failed to serialize observed schema: {e}"));
                fs::write(observed_dir.join("schema.yaml"), &observed_yaml)
                    .unwrap_or_else(|e| panic!("Failed to write observed schema: {e}"));

                let expected_file = Path::new(&case.test_dir).join(EXPECTED_SCHEMA_FILE);
                let expected_yaml = fs::read_to_string(&expected_file)
                    .unwrap_or_else(|e| panic!("Failed to read {}: {e}", expected_file.display()));
                let expected_schema: ResolvedTelemetrySchema =
                    serde_saphyr::from_str(&expected_yaml).unwrap_or_else(|e| {
                        panic!("Failed to deserialize {}: {e}", expected_file.display())
                    });

                assert_resolved_v2_schema_eq(
                    &case.test_dir,
                    &expected_schema,
                    &observed_schema,
                    &observed_yaml,
                );
            }
            TestCaseKind::Errors => {
                let observed_err = match res {
                    Err(e) => e,
                    Ok(_) => {
                        if warnings.is_empty() {
                            panic!(
                                "Test `{}` was expected to fail with errors but succeeded with no warnings",
                                case.test_dir
                            );
                        } else if warnings.len() == 1 {
                            warnings.into_iter().next().unwrap()
                        } else {
                            Error::CompoundError(warnings)
                        }
                    }
                };
                let observed_json = serde_json::to_string(&observed_err)
                    .unwrap_or_else(|e| panic!("Failed to serialize error: {e}"));
                let expected_file = Path::new(&case.test_dir).join(EXPECTED_ERRORS_FILE);
                let expected_json = fs::read_to_string(&expected_file)
                    .unwrap_or_else(|e| panic!("Failed to read {}: {e}", expected_file.display()));

                if case.test_dir == "data/registry-test-v2-6-identity-demote" {
                    // V2 resolution reports unprefixed V2 identifiers rather than V1 `entity.` group IDs.
                    assert!(
                        matches!(
                            &observed_err,
                            Error::EntityRefinementChangedIdentity {
                                refinement_id,
                                r#ref,
                                attribute_id,
                                role: AttributeRole::Descriptive,
                                ..
                            } if refinement_id == "demoting.entity" && r#ref == "base.entity" && attribute_id == "test.attr"
                        ),
                        "Unexpected error for {}: {observed_err:?}",
                        case.test_dir,
                    );
                } else if case.test_dir == "data/registry-test-v2-7-identity-promote" {
                    assert!(
                        matches!(
                            &observed_err,
                            Error::EntityRefinementChangedIdentity {
                                refinement_id,
                                r#ref,
                                attribute_id,
                                role: AttributeRole::Identifying,
                                ..
                            } if refinement_id == "promoting.entity" && r#ref == "base.entity" && attribute_id == "test.attr2"
                        ),
                        "Unexpected error for {}: {observed_err:?}",
                        case.test_dir,
                    );
                } else if case.test_dir == "data/registry-test-v2-8-identity-add" {
                    assert!(
                        matches!(
                            &observed_err,
                            Error::EntityRefinementChangedIdentity {
                                refinement_id,
                                r#ref,
                                attribute_id,
                                role: AttributeRole::Identifying,
                                ..
                            } if refinement_id == "adding.entity" && r#ref == "base.entity" && attribute_id == "test.attr2"
                        ),
                        "Unexpected error for {}: {observed_err:?}",
                        case.test_dir,
                    );
                } else {
                    assert_resolved_errors_eq(&case.test_dir, &expected_json, &observed_json);
                }
            }
        }
    }
}

/// Top-level fixtures under `data/` that are intentionally not covered by a test case.
const SKIP_LIST: &[(&str, &str)] = &[
    (
        "circular-registry-test",
        "Circular dependency cycle between registry_a and registry_b",
    ),
    (
        "dependency-not-found",
        "References a non-existent local dependency directory",
    ),
    (
        "diamond-resolved-test",
        "Requires remote network access to un-mirrored dependency URL",
    ),
    (
        "duplicate-dependency",
        "Declares duplicate dependencies on the same schema URL",
    ),
    (
        "entity-identity-inheritance-unref",
        "Attempting to unref inherited entity identity is an error",
    ),
    (
        "incompatible-version-conflict",
        "Incompatible semver major versions (1.0.0 vs 2.0.0)",
    ),
    (
        "invalid-version-conflict",
        "Invalid semver versions in manifest (\"v1\", \"v2\")",
    ),
    (
        "mandatory-schema-url",
        "V2 manifest missing mandatory dependency schema_url",
    ),
    (
        "registry-test-6-resources",
        "V1 resource group without identity attributes cannot convert to V2 schema",
    ),
    (
        "registry-test-lineage-3",
        "V1 entity group without identity attributes cannot convert to V2 schema",
    ),
    (
        "resolved-conflict-test",
        "Requires remote network access to un-mirrored published URL",
    ),
    (
        "unversioned-conflict",
        "Duplicate unversioned dependency names are fatal",
    ),
];

#[test]
fn test_fixture_coverage_guard() {
    let cases = discover_test_cases();
    let entries = fs::read_dir("data").expect("Failed to read data directory");

    for entry in entries.filter_map(|e| e.ok()) {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().into_owned();
        let dir_path = format!("data/{dir_name}");

        let is_covered = cases
            .iter()
            .any(|c| c.test_dir == dir_path || c.test_dir.starts_with(&format!("{dir_path}/")));

        if !is_covered {
            let is_skipped = SKIP_LIST.iter().any(|(skip, _reason)| skip == &dir_name);
            assert!(
                is_skipped,
                "Top-level directory `{dir_path}` is neither covered by a test case nor listed in SKIP_LIST"
            );
        }
    }
}

/// All valid fixtures targeted for V2 golden schema snapshots.
const VALID_FIXTURES: &[&str] = &[
    "data/compatible-version-conflict-published/main",
    "data/compatible-version-conflict/main",
    "data/correct-provenance/main",
    "data/diamond-conflict/main",
    "data/entity-assoc-imports/legacy_resource",
    "data/entity-assoc-imports/local_private_pair",
    "data/entity-assoc-imports/local_shapes",
    "data/entity-assoc-imports/middle",
    "data/entity-assoc-imports/middle_reexport",
    "data/entity-assoc-imports/middle_refined_signal",
    "data/entity-assoc-imports/middle_refinement_assoc",
    "data/entity-assoc-imports/top_diamond",
    "data/entity-assoc-imports/top_diamond_assoc",
    "data/entity-assoc-imports/top_legacy_import",
    "data/entity-assoc-imports/top_private_first",
    "data/entity-assoc-imports/top_public_first",
    "data/entity-assoc-imports/top_rebind",
    "data/entity-assoc-imports/top_rival_import",
    "data/entity-identity-dependency-role/consumer",
    "data/entity-identity-inheritance",
    "data/inherited-ref-test/base",
    "data/inherited-ref-test/middle",
    "data/inherited-ref-test/refiner",
    "data/multi-registry/app_registry",
    "data/registry-test-1-single-attr-ref",
    "data/registry-test-10-prefix-refs",
    "data/registry-test-11-prefix-refs-extends",
    "data/registry-test-14-annotations",
    "data/registry-test-15-attr-any",
    "data/registry-test-17-metrics-requirement-level",
    "data/registry-test-2-multi-attr-refs",
    "data/registry-test-3-extends",
    "data/registry-test-4-events",
    "data/registry-test-5-metrics",
    "data/registry-test-7-spans",
    "data/registry-test-8-http",
    "data/registry-test-9-metric-extends",
    "data/registry-test-dep-exclusion/migration_consumer",
    "data/registry-test-dep-exclusion/migration_parent",
    "data/registry-test-lineage-0",
    "data/registry-test-lineage-1",
    "data/registry-test-lineage-2",
    "data/registry-test-published-1",
    "data/registry-test-published-2",
    "data/registry-test-v2-1-everything",
    "data/registry-test-v2-2-multifile",
    "data/registry-test-v2-3-both-version",
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
    "data/registry-test-v2-dep/span_registry",
    "data/registry-test-v2-dep/v1_dep_registry",
    "data/signal-name-preservation/v2",
    "data/transitive-provenance/main",
    "data/v1-dependency-syntax/main",
];

#[test]
#[ignore = "explicitly run with: cargo test -p weaver_resolver --test v2_goldens -- --ignored regenerate_v2_goldens"]
fn regenerate_v2_goldens() {
    let mut failures = Vec::new();
    for fixture_dir in VALID_FIXTURES {
        let settings = load_settings(fixture_dir);
        let (res, warnings) = resolve_test_case(fixture_dir, &settings, ResolutionTarget::V1);

        let observed_schema = match res {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{fixture_dir}: resolution failed: {e:?}"));
                continue;
            }
        };

        let observed_yaml = serde_saphyr::to_string(&observed_schema)
            .unwrap_or_else(|e| panic!("Failed to serialize {fixture_dir}: {e}"));

        let expected_schema_path = Path::new(fixture_dir).join(EXPECTED_SCHEMA_FILE);
        if expected_schema_path.exists() {
            let existing_yaml = fs::read_to_string(&expected_schema_path).unwrap_or_else(|e| {
                panic!("Failed to read {}: {e}", expected_schema_path.display())
            });
            let existing_schema: Result<ResolvedTelemetrySchema, _> =
                serde_saphyr::from_str(&existing_yaml);
            match existing_schema {
                Ok(expected) => {
                    let expected_val = serde_json::to_value(&expected).unwrap();
                    let observed_val = serde_json::to_value(&observed_schema).unwrap();
                    if expected_val != observed_val {
                        failures.push(format!(
                            "{fixture_dir}: existing expected-schema.yaml does NOT match current output! Refusing to overwrite."
                        ));
                        continue;
                    }
                }
                Err(e) => {
                    failures.push(format!(
                        "{fixture_dir}: failed to parse existing expected-schema.yaml: {e}"
                    ));
                    continue;
                }
            }
        } else {
            // Create expected-schema.yaml
            fs::write(&expected_schema_path, &observed_yaml).unwrap_or_else(|e| {
                panic!("Failed to write {}: {e}", expected_schema_path.display())
            });
            println!("Created {}", expected_schema_path.display());
        }

        // Handle expected-warnings.json
        let expected_warnings_path = Path::new(fixture_dir).join(EXPECTED_WARNINGS_FILE);
        if !warnings.is_empty() {
            let warnings_json = serde_json::to_string_pretty(&warnings)
                .unwrap_or_else(|e| panic!("Failed to serialize warnings for {fixture_dir}: {e}"));
            fs::write(&expected_warnings_path, warnings_json).unwrap_or_else(|e| {
                panic!("Failed to write {}: {e}", expected_warnings_path.display())
            });
            println!("Wrote {}", expected_warnings_path.display());
        } else if expected_warnings_path.exists() {
            let _ = fs::remove_file(&expected_warnings_path);
        }
    }

    if !failures.is_empty() {
        panic!("Regeneration failed:\n{}", failures.join("\n"));
    }
}
