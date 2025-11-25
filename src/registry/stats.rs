// SPDX-License-Identifier: Apache-2.0

//! Compute stats on a semantic convention registry.

use crate::registry::RegistryArgs;
use crate::util::{load_semconv_specs, resolve_semconv_specs};
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use log::info;
use miette::Diagnostic;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_resolved_schema::registry::{CommonGroupStats, GroupStats};
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_semconv::group::GroupType;
use weaver_semconv::registry::SemConvRegistry;
use weaver_semconv::registry_repo::RegistryRepo;

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
pub(crate) fn command(args: &RegistryStatsArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    info!(
        "Compute statistics on the registry `{}`",
        args.registry.registry
    );

    if args.registry.v2 {
        display_v2(args)?;
    } else {
        display_v1(args)?;
    }

    
    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}


fn display_v2(args: &RegistryStatsArgs) -> Result<(), DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    let registry_path = &args.registry.registry;
    let registry_repo = RegistryRepo::try_new("main", registry_path)?;
    // TODO - v2 way to load things
    let semconv_specs = load_semconv_specs(&registry_repo, args.registry.follow_symlinks)
        .ignore(|e| matches!(e.severity(), Some(miette::Severity::Warning)))
        .into_result_failing_non_fatal()?;
    let mut registry = SemConvRegistry::from_semconv_specs(&registry_repo, semconv_specs)?;
    let resolved_schema = resolve_semconv_specs(&mut registry, args.registry.include_unreferenced)
        .capture_non_fatal_errors(&mut diag_msgs)?;
    let v2_resolved_schema = match weaver_resolved_schema::v2::ResolvedTelemetrySchema::try_from(resolved_schema) {
        Ok(schema) => schema,
        Err(e) => {
            // TODO - add error to diag_msgs.
            return Err(diag_msgs);
        },
    };
    display_schema_stats_v2(&v2_resolved_schema);
    
    Ok(())
}

fn display_v1(args: &RegistryStatsArgs) -> Result<(), DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    let registry_path = &args.registry.registry;
    let registry_repo = RegistryRepo::try_new("main", registry_path)?;

    
    // Load the semantic convention registry into a local cache.
    let semconv_specs = load_semconv_specs(&registry_repo, args.registry.follow_symlinks)
        .ignore(|e| matches!(e.severity(), Some(miette::Severity::Warning)))
        .into_result_failing_non_fatal()?;
    let mut registry = SemConvRegistry::from_semconv_specs(&registry_repo, semconv_specs)?;

    display_semconv_registry_stats(&registry);

    // Resolve the semantic convention registry.
    let resolved_schema = resolve_semconv_specs(&mut registry, args.registry.include_unreferenced)
        .capture_non_fatal_errors(&mut diag_msgs)?;

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    display_schema_stats(&resolved_schema);
    Ok(())
}

fn display_semconv_registry_stats(semconv_registry: &SemConvRegistry) {
    let stats = semconv_registry.stats();
    println!("Semantic Convention Registry Stats:");
    println!("  - Total number of files: {}", stats.file_count);
}

fn display_schema_stats_v2(schema: &weaver_resolved_schema::v2::ResolvedTelemetrySchema) {
    let stats = schema.stats();
    // TODO - grab stdout lock.
    println!("Resolved Telemetry Schema Stats:");
    println!("Registry");
    // Attribute Stats
    println!("- Attributes");
    println!("  - count: {}", stats.registry.attributes.attribute_count);
    println!("  - deprecated: {}", stats.registry.attributes.deprecated_count);
    println!("  - type breakdown: ");
    for (atype, count) in stats.registry.attributes.attribute_type_breakdown.iter() {
        println!("    - {atype}: {count}");
    }
    println!("  - stability breakdown: ");
    for (stability, count) in stats.registry.attributes.stability_breakdown.iter() {
        println!("    - {stability}: {count}");
    }
    println!("- Attribute Groups");
    println!("  TODO");
    // Entity stats
    println!("- Entities");
    println!("  - count: {}", stats.registry.entities.common.count);
    println!("  - deprecated: {}", stats.registry.entities.common.deprecated_count);
    println!("  - stability breakdown: ");
    for (stability, count) in stats.registry.entities.common.stability_breakdown.iter() {
        println!("    - {stability}: {count}");
    }
    println!("  - total with note: {}", stats.registry.entities.common.total_with_note);
    println!("   - entity types count: {}", stats.registry.entities.entity_types.len());
    println!("   - entity identity length distribution: ");
    // TODO - sort by length.
    for (length, count) in stats.registry.entities.entity_identity_length_distribution.iter() {
        println!("      - {length}: {count}");
    }
    // Event stats
    println!("- Events");
    println!("  - count: {}", stats.registry.events.common.count);
    println!("  - deprecated: {}", stats.registry.events.common.deprecated_count);
    println!("  - stability breakdown: ");
    for (stability, count) in stats.registry.events.common.stability_breakdown.iter() {
        println!("    - {stability}: {count}");
    }
    println!("  - total with note: {}", stats.registry.events.common.total_with_note);
    // Metric stats
    println!("- Metrics");
    println!("  - count: {}", stats.registry.metrics.common.count);
    println!("  - deprecated: {}", stats.registry.metrics.common.deprecated_count);
    println!("  - stability breakdown: ");
    for (stability, count) in stats.registry.metrics.common.stability_breakdown.iter() {
        println!("    - {stability}: {count}");
    }
    println!("  - total with note: {}", stats.registry.metrics.common.total_with_note);
    println!("  - instrument breakdown: ");
    // TODO - sort by count
    for (instrument, count) in stats.registry.metrics.instrument_breakdown.iter() {
        println!("    - {instrument}: {count}");
    }
    println!("  - unit breakdown: ");
    // TODO - sort by count
    for (unit, count) in stats.registry.metrics.unit_breakdown.iter() {
        println!("    - {unit}: {count}");
    }
    // Span stats
    println!("- Spans");
    println!("  - count: {}", stats.registry.spans.common.count);
    println!("  - deprecated: {}", stats.registry.spans.common.deprecated_count);
    println!("  - stability breakdown: ");
    for (stability, count) in stats.registry.spans.common.stability_breakdown.iter() {
        println!("    - {stability}: {count}");
    }
    println!("  - total with note: {}", stats.registry.spans.common.total_with_note);
    println!("  - span kind breakdown: ");
    // TODO - sort by count
    for (span_kind, count) in stats.registry.spans.span_kind_breakdown.iter() {
        println!("    - {span_kind:?}: {count}");
    }
    
}

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
                        println!("        - {instrument}: {count}");
                    }
                    println!("      - Unit breakdown:");
                    for (unit, count) in unit_breakdown.iter() {
                        println!("        - {unit}: {count}");
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
                GroupStats::Entity { common_stats } => {
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
                        println!("        - {span_kind:#?}: {count}");
                    }
                }
                GroupStats::Undefined { common_stats } => {
                    display_common_group_stats(group_type, common_stats);
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
        println!("      - {attribute_type}: {count}");
    }
    println!("    - Requirement levels breakdown:");
    for (requirement_level, count) in catalog_stats.requirement_level_breakdown.iter() {
        println!("      - {requirement_level}: {count}");
    }
    if !catalog_stats.stability_breakdown.is_empty() {
        println!(
            "    - Stability breakdown ({}%):",
            catalog_stats.stability_breakdown.values().sum::<usize>() * 100
                / catalog_stats.attribute_count
        );
        for (stability, count) in catalog_stats.stability_breakdown.iter() {
            println!("      - {stability}: {count}");
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
    if common_stats.count > 0 {
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
    }
    if !common_stats.stability_breakdown.is_empty() {
        println!(
            "      - Stability breakdown ({}%):",
            common_stats.stability_breakdown.values().sum::<usize>() * 100 / common_stats.count
        );
        for (stability, count) in common_stats.stability_breakdown.iter() {
            println!("        - {stability}: {count}");
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
