// SPDX-License-Identifier: Apache-2.0

//! Check the gap between a semantic convention registry and an OTLP traffic.

use crate::registry::otlp::{listen_otlp_requests, OtlpRequest};
use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::prepare_main_registry;
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use std::collections::HashSet;
use weaver_forge::registry::ResolvedGroup;
use std::time::Duration;
use weaver_semconv::group::GroupType;
use weaver_semconv::requirement_level::RequirementLevel;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;

/// Parameters for the `registry live-check` sub-command
#[derive(Debug, Args)]
pub struct CheckRegistryArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Address used by the gRPC OTLP listener.
    #[clap(long, default_value = "0.0.0.0")]
    pub otlp_grpc_address: String,

    /// Port used by the gRPC OTLP listener.
    #[clap(long, default_value = "4317", short = 'p')]
    pub otlp_grpc_port: u16,

    /// Port used by the HTTP admin port (endpoints: /stop).
    #[clap(long, default_value = "4320", short = 'a')]
    pub admin_port: u16,

    /// Max inactivity time in seconds before stopping the listener.
    #[clap(long, default_value = "10", short = 't')]
    pub inactivity_timeout: u64,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Check the conformance level of an OTLP stream against a semantic convention registry.
///
/// This command starts an OTLP listener and compares each received OTLP message with the
/// registry provided as a parameter. When the command is stopped (see stop conditions),
/// a conformance/coverage report is generated. The purpose of this command is to be used
/// in a CI/CD pipeline to validate the telemetry stream from an application or service
/// against a registry.
///
/// The currently supported stop conditions are: CTRL+C (SIGINT), SIGHUP, the HTTP /stop
/// endpoint, and a maximum duration of no OTLP message reception.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &CheckRegistryArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let mut diag_msgs = DiagnosticMessages::empty();
    let policy = PolicyArgs::skip();
    let otlp_requests = listen_otlp_requests(
        args.otlp_grpc_address.as_str(),
        args.otlp_grpc_port,
        args.admin_port,
        Duration::from_secs(args.inactivity_timeout),
        logger.clone(),
    )?;

    // @ToDo Use the following resolved registry to check the level of compliance of the incoming OTLP messages
    let (resolved_registry, _) =
        prepare_main_registry(&args.registry, &policy, logger.clone(), &mut diag_msgs)?;

    logger.loading(&format!(
        "Checking OTLP traffic on port {}.",
        args.otlp_grpc_port
    ));

    let registry_data = resolved_registry.groups;

    // use a hashset to keep track of what registry items are seen in the OTLP stream
    let mut registry_set: HashSet<&ResolvedGroup> = HashSet::new();
    for registry_item in registry_data.iter() {
        registry_set.insert(registry_item);
    }
    let mut report: Vec<String> = Vec::new();
    let registry_data_seen: HashSet<&ResolvedGroup> = HashSet::new();
    // @ToDo Implement the checking logic d
    for otlp_request in otlp_requests {
        match otlp_request {
            OtlpRequest::Logs(_logs) => {
                for logs in _logs.resource_logs {
                    for scope_logs in logs.scope_logs {
                        for log_data in scope_logs.log_records {
                            // Check if log attributes match registry definition
                            if let Some(registry_log) = registry_data.iter().find(|m| m.r#type == GroupType::Event && m.name == Some(log_data.event_name)) {
                                // compare the attributes stored in _registry_log.attributes to log_data.attributes
                                registry_data_seen.insert(registry_log);
                                // Validate log event name matches registry definition
                                let mut attrs: HashSet<String> = log_data.attributes.iter().map(|attribute_key_value| attribute_key_value.key ).collect();
                                for attribute in registry_log.attributes.iter() {
                                    if (attribute.requirement_level == RequirementLevel::Basic) && !(attrs.contains(&attribute.name)) {
                                        report.push(format!("Log event'{}' has missing required attribute '{}'", log_data.event_name, attribute.name));
                                    }
                                }
                            } else {
                                // Report missing log event in registry
                                report.push(format!("Log event '{}' not found in registry",log_data.event_name));
                            }
                        }
                    }
                }
                println!("Logs Request received");
            }
            OtlpRequest::Metrics(_metrics) => {
                for metric in _metrics.resource_metrics {
                    for scope_metric in metric.scope_metrics {
                        for metric_data in scope_metric.metrics {
                            if let Some(registry_metric) = registry_data.iter().find(|m| m.r#type == GroupType::Metric && m.metric_name == Some(metric_data.name)) {
                                registry_data_seen.insert(registry_metric);
                                if registry_metric.unit != Some(metric_data.unit) {
                                    // Report mismatched unit
                                    report.push(format!("Metric {} has a mismatched unit", metric_data.name));
                                }
                            } else {

                                // Report missing metric in registry
                                report.push(format!("Metric {} not found in registry", metric_data.name));
                            }
                        }
                    }
                }
                println!("Metrics Request received");
            }
            OtlpRequest::Traces(_traces) => {
                for traces in _traces.resource_spans {
                    for scope_traces in traces.scope_spans {
                        for span_data in scope_traces.spans {
                            // Check if span attributes match registry definition
                            if let Some(registry_span) = registry_data.iter().find(|m| m.r#type == GroupType::Span && m.id.replace("span.", "") == Some(span_data.name)) {
                                // compare the attributes stored in _registry_span.attributes to span_data.attributes
                                // if they do match mark the span as seen
                                registry_data_seen.insert(registry_span);
                                // store all attributes in a hashset to quickly check for attribute existence
                                let mut attrs = span_data.attributes.iter().map(|attribute_key_value| attribute_key_value.key ).collect();

                                for attribute in registry_span.attributes.iter() {
                                    if (attribute.requirement_level == RequirementLevel::Basic) && !attrs.contains(&attribute.name) {
                                        report.push(format!("Span event '{}' has missing required attribute '{}'", span_data.name, attribute.name));
                                    }
                                }

                            } else {

                                // Report missing span in registry
                                report.push(format!("Span {} not found in registry", span_data.name));
                            }
                        }
                    }
                }
                println!("Trace Request received");
            }
            OtlpRequest::Stop(reason) => {
                logger.warn(&format!("Stopping the listener, reason: {}", reason));

                // difference between hashset registry_set - registry_data_seen = missing items in registry_data_seen
                // difference between registry_data_seen - registry_set = missing items in registry_set
                
                let registry_missing = registry_set.difference(&registry_data_seen).collect();
                let missing_items_in_registry_data_seen: HashSet<&ResolvedGroup> =
                    registry_data_seen.d
                
                // loop through registry_set to print out ids that are not in registry_data_seen
                for registry_item in registry_set.iter() {
                    println!("Registry item {} not found in registry", registry_item.id);
                }
                
                // iter through _report and print out each error
                for report in report {
                    println!("{}", report);
                }
                println!("Stopping the listener, reason: {}", reason);
                break;
            }
            OtlpRequest::Error(error) => {
                diag_msgs.extend(DiagnosticMessages::from_error(error));
                break;
            }
        }
    }

    if diag_msgs.has_error() {
        return Err(diag_msgs);
    }

    logger.success("OTLP requests received and checked.");

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}

