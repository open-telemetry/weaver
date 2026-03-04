// SPDX-License-Identifier: Apache-2.0

//! Generates a semantic convention registry file by inferring the schema from OTLP messages.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use clap::Args;
use log::info;
use weaver_infer::AccumulatedSamples;
use weaver_live_check::sample_resource::SampleResource;
use weaver_live_check::sample_span::{SampleSpan, SampleSpanEvent};
use weaver_live_check::Sample;

use super::otlp::conversion::{
    otlp_log_record_to_sample_log, otlp_metric_to_sample, sample_attribute_from_key_value,
    span_kind_from_otlp_kind, status_from_otlp_status,
};
use super::otlp::grpc_stubs::proto::resource::v1::Resource;
use super::otlp::{listen_otlp_requests, OtlpRequest};
use crate::{DiagnosticArgs, ExitDirectives};
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::log_success;

/// Parameters for the `registry infer` sub-command
#[derive(Debug, Args)]
pub struct RegistryInferArgs {
    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,

    /// Output folder for generated YAML files.
    #[arg(short, long, default_value = "./inferred-registry/")]
    output: PathBuf,

    /// Address used by the gRPC OTLP listener.
    #[arg(long, default_value = "0.0.0.0")]
    grpc_address: String,

    /// Port used by the gRPC OTLP listener.
    #[arg(long, default_value = "4317")]
    grpc_port: u16,

    /// Port used by the HTTP admin server (endpoints: /stop).
    #[arg(long, default_value = "8080")]
    admin_port: u16,

    /// Seconds of inactivity before auto-stop (0 = never).
    #[arg(long, default_value = "60")]
    inactivity_timeout: u64,
}

/// Accumulates resource attributes from an OTLP Resource into the accumulator.
fn accumulate_resource(resource: Option<Resource>, accumulator: &mut AccumulatedSamples) {
    if let Some(resource) = resource {
        let mut sample_resource = SampleResource {
            attributes: Vec::new(),
            live_check_result: None,
        };
        for attribute in resource.attributes {
            sample_resource
                .attributes
                .push(sample_attribute_from_key_value(&attribute));
        }
        accumulator.add_sample(Sample::Resource(sample_resource));
    }
}

fn process_otlp_request(request: OtlpRequest, accumulator: &mut AccumulatedSamples) -> bool {
    match request {
        OtlpRequest::Logs(logs) => {
            for resource_log in logs.resource_logs {
                accumulate_resource(resource_log.resource, accumulator);

                for scope_log in resource_log.scope_logs {
                    for log_record in scope_log.log_records {
                        let sample_log = otlp_log_record_to_sample_log(&log_record);
                        accumulator.add_sample(Sample::Log(sample_log));
                    }
                }
            }
            true
        }
        OtlpRequest::Metrics(metrics) => {
            for resource_metric in metrics.resource_metrics {
                accumulate_resource(resource_metric.resource, accumulator);

                for scope_metric in resource_metric.scope_metrics {
                    for metric in scope_metric.metrics {
                        let sample_metric = otlp_metric_to_sample(metric);
                        accumulator.add_sample(Sample::Metric(sample_metric));
                    }
                }
            }
            true
        }
        OtlpRequest::Traces(trace) => {
            for resource_span in trace.resource_spans {
                accumulate_resource(resource_span.resource, accumulator);

                for scope_span in resource_span.scope_spans {
                    for span in scope_span.spans {
                        let span_kind = span.kind();
                        let mut sample_span = SampleSpan {
                            name: span.name,
                            kind: span_kind_from_otlp_kind(span_kind),
                            status: status_from_otlp_status(span.status),
                            attributes: Vec::new(),
                            span_events: Vec::new(),
                            span_links: Vec::new(),
                            live_check_result: None,
                            resource: None,
                        };
                        for attribute in span.attributes {
                            sample_span
                                .attributes
                                .push(sample_attribute_from_key_value(&attribute));
                        }
                        for event in span.events {
                            let mut sample_event = SampleSpanEvent {
                                name: event.name,
                                attributes: Vec::new(),
                                live_check_result: None,
                            };
                            for attribute in event.attributes {
                                sample_event
                                    .attributes
                                    .push(sample_attribute_from_key_value(&attribute));
                            }
                            sample_span.span_events.push(sample_event);
                        }
                        accumulator.add_sample(Sample::Span(sample_span));
                    }
                }
            }
            true
        }
        OtlpRequest::Stop(signal) => {
            info!("Received stop signal: {}", signal);
            false
        }
        OtlpRequest::Error(e) => {
            info!("Received error: {:?}", e);
            true // Continue processing
        }
    }
}

/// Infer a semantic convention registry from OTLP telemetry.
pub(crate) fn command(args: &RegistryInferArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    log::warn!(
        "The `registry infer` command is experimental and not yet stable. \
        The generated schema format, command options, and output may change in future versions."
    );

    info!("Weaver Registry Infer");
    info!(
        "Starting OTLP gRPC server on {}:{}",
        args.grpc_address, args.grpc_port
    );

    // Start the OTLP gRPC server and get an iterator of requests
    let (requests, _report_sender) = listen_otlp_requests(
        &args.grpc_address,
        args.grpc_port,
        args.admin_port,
        Duration::from_secs(args.inactivity_timeout),
    )
    .map_err(DiagnosticMessages::from)?;

    info!("OTLP gRPC server started. Waiting for telemetry...");
    info!(
        "To stop: press CTRL+C, send SIGHUP, or POST to http://localhost:{}/stop",
        args.admin_port
    );

    // Accumulate samples
    let mut accumulator = AccumulatedSamples::new();

    for request in requests {
        if !process_otlp_request(request, &mut accumulator) {
            break;
        }
    }

    let (resources, spans, metrics, events) = accumulator.stats();
    info!(
        "OTLP receiver stopped. Accumulated: {} resource attrs, {} spans, {} metrics, {} events",
        resources, spans, metrics, events
    );

    if accumulator.is_empty() {
        info!("No telemetry data received. No YAML file generated.");
    } else {
        // Create output directory
        fs::create_dir_all(&args.output).map_err(|e| {
            DiagnosticMessages::from(super::otlp::Error::OtlpError {
                error: format!("Failed to create output directory: {}", e),
            })
        })?;

        // Generate YAML
        let registry = accumulator.to_semconv_spec();
        let yaml = serde_yaml::to_string(&registry).map_err(|e| {
            DiagnosticMessages::from(super::otlp::Error::OtlpError {
                error: format!("Failed to serialize YAML: {}", e),
            })
        })?;

        // Write to file
        let output_path = args.output.join("registry.yaml");
        fs::write(&output_path, yaml).map_err(|e| {
            DiagnosticMessages::from(super::otlp::Error::OtlpError {
                error: format!("Failed to write file: {}", e),
            })
        })?;

        info!("Generated registry file: {:?}", output_path);
    }

    log_success("Registry infer completed");

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}
