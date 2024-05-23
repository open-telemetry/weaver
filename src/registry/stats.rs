// SPDX-License-Identifier: Apache-2.0

//! Compute stats on a semantic convention registry.

use crate::registry::{
    load_semconv_specs, resolve_semconv_specs, semconv_registry_path_from, RegistryArgs,
};
use crate::DiagnosticArgs;
use clap::Args;
use weaver_cache::Cache;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_resolved_schema::registry::{CommonGroupStats, GroupStats};
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_semconv::group::GroupType;
use weaver_semconv::registry::SemConvRegistry;

/// Parameters for the `registry stats` sub-command
#[derive(Debug, Args)]
pub struct RegistryStatsArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Compute stats on a semantic convention registry.
#[cfg(not(tarpaulin_include))]
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistryStatsArgs,
) -> Result<(), DiagnosticMessages> {
    logger.loading(&format!(
        "Compute statistics on the registry `{}`",
        args.registry.registry
    ));

    let registry_id = "default";
    let registry_path =
        semconv_registry_path_from(&args.registry.registry, &args.registry.registry_git_sub_dir);

    // Load the semantic convention registry into a local cache.
    let semconv_specs = load_semconv_specs(&registry_path, cache, logger.clone())?;
    let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);

    display_semconv_registry_stats(&registry);

    // Resolve the semantic convention registry.
    let resolved_schema = resolve_semconv_specs(&mut registry, logger)?;

    display_schema_stats(&resolved_schema);
    Ok(())
}

#[cfg(not(tarpaulin_include))]
fn display_semconv_registry_stats(semconv_registry: &SemConvRegistry) {
    let stats = semconv_registry.stats();
    println!("Semantic Convention Registry Stats:");
    println!("  - Total number of files: {}", stats.file_count);
}

#[cfg(not(tarpaulin_include))]
fn display_schema_stats(schema: &ResolvedTelemetrySchema) {
    let stats = schema.stats();
    println!("Resolved Telemetry Schema Stats:");
    let mut total_number_of_attributes = 0;
    for registry_stats in stats.registry_stats.iter() {
        println!("Registry");
        println!("  - {} groups", registry_stats.group_count);
        for (group_type, group_stats) in registry_stats.group_breakdown.iter() {
            match group_stats {
                GroupStats::AttributeGroup { common_stats } => {
                    display_common_group_stats(group_type, common_stats);
                    total_number_of_attributes += common_stats.total_attribute_count;
                }
                GroupStats::Metric {
                    common_stats,
                    metric_names,
                    instrument_breakdown,
                    unit_breakdown,
                } => {
                    display_common_group_stats(group_type, common_stats);
                    total_number_of_attributes += common_stats.total_attribute_count;
                    println!(
                        "      - Distinct number of metric names: {}",
                        metric_names.len()
                    );
                    println!("      - Instrument breakdown:");
                    for (instrument, count) in instrument_breakdown.iter() {
                        println!("        - {}: {}", instrument, count);
                    }
                    println!("      - Unit breakdown:");
                    for (unit, count) in unit_breakdown.iter() {
                        println!("        - {}: {}", unit, count);
                    }
                }
                GroupStats::MetricGroup { common_stats } => {
                    display_common_group_stats(group_type, common_stats);
                    total_number_of_attributes += common_stats.total_attribute_count;
                }
                GroupStats::Event { common_stats } => {
                    display_common_group_stats(group_type, common_stats);
                    total_number_of_attributes += common_stats.total_attribute_count;
                }
                GroupStats::Resource { common_stats } => {
                    display_common_group_stats(group_type, common_stats);
                    total_number_of_attributes += common_stats.total_attribute_count;
                }
                GroupStats::Scope { common_stats } => {
                    display_common_group_stats(group_type, common_stats);
                    total_number_of_attributes += common_stats.total_attribute_count;
                }
                GroupStats::Span {
                    common_stats,
                    span_kind_breakdown,
                } => {
                    display_common_group_stats(group_type, common_stats);
                    total_number_of_attributes += common_stats.total_attribute_count;
                    println!("      - Span kind breakdown:");
                    for (span_kind, count) in span_kind_breakdown.iter() {
                        println!("        - {:#?}: {}", span_kind, count);
                    }
                }
            }
        }
    }

    let catalog_stats = &stats.catalog_stats;
    println!("Shared Catalog (after resolution and deduplication):");
    if total_number_of_attributes > 0 {
        println!(
            "  - Number of deduplicated attributes: {} ({}%)",
            catalog_stats.attribute_count,
            catalog_stats.attribute_count * 100 / total_number_of_attributes
        );
    }
    println!("    - Attribute types breakdown:");
    for (attribute_type, count) in catalog_stats.attribute_type_breakdown.iter() {
        println!("      - {}: {}", attribute_type, count);
    }
    println!("    - Requirement levels breakdown:");
    for (requirement_level, count) in catalog_stats.requirement_level_breakdown.iter() {
        println!("      - {}: {}", requirement_level, count);
    }
    if !catalog_stats.stability_breakdown.is_empty() {
        println!(
            "    - Stability breakdown ({}%):",
            catalog_stats.stability_breakdown.values().sum::<usize>() * 100
                / catalog_stats.attribute_count
        );
        for (stability, count) in catalog_stats.stability_breakdown.iter() {
            println!("      - {}: {}", stability, count);
        }
    }
    if catalog_stats.deprecated_count > 0 {
        println!(
            "    - Total number of deprecated attributes: {} ({}%)",
            catalog_stats.deprecated_count,
            catalog_stats.deprecated_count * 100 / catalog_stats.attribute_count
        );
    }
}

#[cfg(not(tarpaulin_include))]
fn display_common_group_stats(group_type: &GroupType, common_stats: &CommonGroupStats) {
    println!("    - {} {:#?}s", common_stats.count, group_type);
    println!(
        "      - Total number of attributes: {}",
        common_stats.total_attribute_count
    );
    println!(
        "        - [(attributes card: frequency), ...]: [{}]",
        common_stats
            .attribute_card_breakdown
            .iter()
            .map(|(card, count)| format!("{}: {}", *card, *count))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "      - Number of group with a prefix: {} ({}%)",
        common_stats.total_with_prefix,
        common_stats.total_with_prefix * 100 / common_stats.count
    );
    println!(
        "      - Number of group with a note: {} ({}%)",
        common_stats.total_with_note,
        common_stats.total_with_note * 100 / common_stats.count
    );
    if !common_stats.stability_breakdown.is_empty() {
        println!(
            "      - Stability breakdown ({}%):",
            common_stats.stability_breakdown.values().sum::<usize>() * 100 / common_stats.count
        );
        for (stability, count) in common_stats.stability_breakdown.iter() {
            println!("        - {}: {}", stability, count);
        }
    }
    if common_stats.deprecated_count > 0 {
        println!(
            "      - Total number of deprecated groups: {} ({}%)",
            common_stats.deprecated_count,
            common_stats.deprecated_count * 100 / common_stats.count
        );
    }
}
