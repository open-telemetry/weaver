// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the resolution process.

use miette::Diagnostic;

use weaver_common::vdir::VirtualDirectoryPath;
use weaver_resolver::SchemaResolver;
use weaver_semconv::{manifest::SchemaUrl, registry_repo::RegistryRepo};

/// The URL of the official semantic convention registry.
const SEMCONV_REGISTRY_URL: &str = "https://github.com/open-telemetry/semantic-conventions.git";
/// The directory name of the official semantic convention registry.
const SEMCONV_REGISTRY_MODEL: &str = "model";

/// This test checks the CLI interface for the registry generate command.
/// This test doesn't count for the coverage report as it runs a separate process.
///
/// Test the resolution process for the official semantic convention registry.
/// Success criteria:
/// - All semconv files downloaded from the official semconv repo.
/// - The parsing process should not fail.
/// - The resolution process should not fail.
/// - No warn or error messages should be reported by the logger.
#[test]
fn test_cli_interface() {
    let log = weaver_common::TestLog::new();

    // Load the official semantic convention registry into a local cache.
    // No parsing errors should be observed.
    let registry_path = VirtualDirectoryPath::GitRepo {
        url: SEMCONV_REGISTRY_URL.to_owned(),
        sub_folder: Some(SEMCONV_REGISTRY_MODEL.to_owned()),
        refspec: None,
    };

    let schema_url = Some(SchemaUrl("https://opelemetry.io/schemas/1.40.0".to_owned()));
    let registry_repo =
        RegistryRepo::try_new(schema_url, &registry_path)
            .unwrap_or_else(|e| {
                panic!("Failed to create the registry repo, error: {e}");
            });
    let loaded = SchemaResolver::load_semconv_repository(registry_repo, false)
        .ignore(|e| matches!(e.severity(), Some(miette::Severity::Warning)))
        .into_result_failing_non_fatal()
        .unwrap_or_else(|e| {
            panic!("Failed to load the semantic convention specs, error: {e}");
        });
    // Check if the logger has reported any warnings or errors.
    assert_eq!(log.warn_count(), 0);
    assert_eq!(log.error_count(), 0);

    // Resolve the official semantic convention registry.
    let resolved_registry = SchemaResolver::resolve(loaded, false)
        .into_result_failing_non_fatal()
        .unwrap_or_else(|e| {
            panic!("Failed to resolve the official semantic convention registry, error: {e}");
        })
        .registry;

    // The number of semconv groups is fluctuating, so we can't check for a
    // specific number, but we can check if there are any groups at all.
    assert!(!resolved_registry.groups.is_empty());

    // Check if the logger has reported any warnings or errors.
    assert_eq!(log.warn_count(), 0);
    assert_eq!(log.error_count(), 0);
}
