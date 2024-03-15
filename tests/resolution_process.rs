// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the resolution process.

use weaver_cache::Cache;
use weaver_logger::{Logger, TestLogger};
use weaver_resolver::attribute::AttributeCatalog;
use weaver_resolver::registry::resolve_semconv_registry;
use weaver_resolver::SchemaResolver;

/// The URL of the official semantic convention registry.
const SEMCONV_REGISTRY_URL: &str = "https://github.com/open-telemetry/semantic-conventions.git";
/// The directory name of the official semantic convention registry.
const SEMCONV_REGISTRY_MODEL: &str = "model";

/// Test the resolution process for the official semantic convention registry.
/// Success criteria:
/// - All semconv files downloaded from the official semconv repo.
/// - The parsing process should not fail.
/// - The resolution process should not fail.
/// - No warn or error messages should be reported by the logger.
#[test]
fn test_semconv_registry_resolution() {
    let log = TestLogger::new();
    let cache = Cache::try_new().unwrap_or_else(|e| {
        _=log.error(&e.to_string());
        panic!("Failed to create the git cache repo, error: {e}");
    });

    let registry_id = "default";

    // Load the official semantic convention registry into a local cache.
    // No parsing errors should be observed.
    let semconv_specs = SchemaResolver::load_semconv_registry(
        registry_id,
        SEMCONV_REGISTRY_URL.to_string(),
        Some(SEMCONV_REGISTRY_MODEL.to_string()),
        &cache,
        log.clone(),
    )
    .unwrap_or_else(|e| {
        panic!("Failed to load and parse the official semantic convention registry, error: {e}");
    });

    // Check if the logger has reported any warnings or errors.
    assert_eq!(log.warn_count(), 0);
    assert_eq!(log.error_count(), 0);

    // Resolve the official semantic convention registry.
    let mut attr_catalog = AttributeCatalog::default();
    let resolved_registry =
        resolve_semconv_registry(&mut attr_catalog, SEMCONV_REGISTRY_URL, &semconv_specs)
            .unwrap_or_else(|e| {
                panic!("Failed to resolve the official semantic convention registry, error: {e}");
            });

    // The number of semconv groups is fluctuating, so we can't check for a
    // specific number, but we can check if there are any groups at all.
    assert!(!resolved_registry.groups.is_empty());

    // Check if the logger has reported any warnings or errors.
    assert_eq!(log.warn_count(), 0);
    assert_eq!(log.error_count(), 0);
}

/// Test the resolution process for the official Telemetry Schema.
/// Success criteria: The resolution process should not fail.
#[test]
fn test_telemetry_schema_resolution() {
    // ToDo once the official Application Telemetry Schema is approved and implemented by this project.
}
