// SPDX-License-Identifier: Apache-2.0

//! Generates a semantic convention registry file by inferring the schema from OTLP messages.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use clap::Args;
use log::info;
use serde::Serialize;
use serde_json::Value;
use weaver_live_check::sample_attribute::SampleAttribute;
use weaver_live_check::sample_metric::{SampleInstrument, SampleMetric};
use weaver_live_check::sample_resource::SampleResource;
use weaver_live_check::sample_span::{SampleSpan, SampleSpanEvent};
use weaver_live_check::Sample;
use weaver_semconv::attribute::PrimitiveOrArrayTypeSpec;
use weaver_semconv::group::{InstrumentSpec, SpanKindSpec};

use super::otlp::conversion::{
    otlp_log_record_to_sample_log, otlp_metric_to_sample, sample_attribute_from_key_value,
    span_kind_from_otlp_kind, status_from_otlp_status,
};
use super::otlp::{listen_otlp_requests, OtlpRequest};
use crate::{DiagnosticArgs, ExitDirectives};
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::log_success;

const MAX_EXAMPLES: usize = 5;

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

/// Accumulated attribute with examples
#[derive(Debug, Clone)]
struct AccumulatedAttribute {
    name: String,
    attr_type: Option<PrimitiveOrArrayTypeSpec>,
    examples: Vec<Value>,
}

impl AccumulatedAttribute {
    fn new(name: String, attr_type: Option<PrimitiveOrArrayTypeSpec>) -> Self {
        Self {
            name,
            attr_type,
            examples: Vec::new(),
        }
    }

    fn add_example(&mut self, value: &Option<Value>) {
        if let Some(v) = value {
            if self.examples.len() < MAX_EXAMPLES && !self.examples.contains(v) {
                self.examples.push(v.clone());
            }
        }
    }
}

#[derive(Debug, Clone)]
struct AccumulatedSpan {
    name: String,
    kind: SpanKindSpec,
    attributes: HashMap<String, AccumulatedAttribute>,
    events: HashMap<String, AccumulatedEvent>,
}

impl AccumulatedSpan {
    fn new(name: String, kind: SpanKindSpec) -> Self {
        Self {
            name,
            kind,
            attributes: HashMap::new(),
            events: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct AccumulatedMetric {
    name: String,
    instrument: Option<InstrumentSpec>,
    unit: String,
    attributes: HashMap<String, AccumulatedAttribute>,
}

impl AccumulatedMetric {
    fn new(name: String, instrument: Option<InstrumentSpec>, unit: String) -> Self {
        Self {
            name,
            instrument,
            unit,
            attributes: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct AccumulatedEvent {
    name: String,
    attributes: HashMap<String, AccumulatedAttribute>,
}

impl AccumulatedEvent {
    fn new(name: String) -> Self {
        Self {
            name,
            attributes: HashMap::new(),
        }
    }
}

/// Main accumulator for all samples
#[derive(Debug, Default)]
struct AccumulatedSamples {
    resources: HashMap<String, AccumulatedAttribute>,
    spans: HashMap<String, AccumulatedSpan>,
    metrics: HashMap<String, AccumulatedMetric>,
    events: HashMap<String, AccumulatedEvent>,
}

impl AccumulatedSamples {
    fn new() -> Self {
        Self::default()
    }

    fn add_sample(&mut self, sample: Sample) {
        match sample {
            Sample::Resource(resource) => self.add_resource(resource),
            Sample::Span(span) => self.add_span(span),
            Sample::Metric(metric) => self.add_metric(metric),
            Sample::Log(log) => self.add_event(log.event_name, log.attributes),
            Sample::Attribute(attr) => self.add_resource_attribute(attr),
            _ => {} // Ignore other sample types for now
        }
    }

    fn add_resource(&mut self, resource: SampleResource) {
        for attr in resource.attributes {
            self.add_resource_attribute(attr);
        }
    }

    fn add_resource_attribute(&mut self, attr: SampleAttribute) {
        let entry = self
            .resources
            .entry(attr.name.clone())
            .or_insert_with(|| AccumulatedAttribute::new(attr.name.clone(), attr.r#type.clone()));
        entry.add_example(&attr.value);
    }

    fn add_span(&mut self, span: SampleSpan) {
        let entry = self
            .spans
            .entry(span.name.clone())
            .or_insert_with(|| AccumulatedSpan::new(span.name.clone(), span.kind.clone()));

        for attr in span.attributes {
            let attr_entry = entry
                .attributes
                .entry(attr.name.clone())
                .or_insert_with(|| {
                    AccumulatedAttribute::new(attr.name.clone(), attr.r#type.clone())
                });
            attr_entry.add_example(&attr.value);
        }

        for event in span.span_events {
            let event_entry = entry
                .events
                .entry(event.name.clone())
                .or_insert_with(|| AccumulatedEvent::new(event.name.clone()));

            for attr in event.attributes {
                let attr_entry = event_entry
                    .attributes
                    .entry(attr.name.clone())
                    .or_insert_with(|| {
                        AccumulatedAttribute::new(attr.name.clone(), attr.r#type.clone())
                    });
                attr_entry.add_example(&attr.value);
            }
        }
    }

    fn add_metric(&mut self, metric: SampleMetric) {
        let instrument = match &metric.instrument {
            SampleInstrument::Supported(i) => Some(i.clone()),
            SampleInstrument::Unsupported(_) => None,
        };

        let entry = self.metrics.entry(metric.name.clone()).or_insert_with(|| {
            AccumulatedMetric::new(metric.name.clone(), instrument, metric.unit.clone())
        });

        if let Some(data_points) = metric.data_points {
            use weaver_live_check::sample_metric::DataPoints;
            match data_points {
                DataPoints::Number(points) => {
                    for point in points {
                        for attr in point.attributes {
                            let attr_entry = entry
                                .attributes
                                .entry(attr.name.clone())
                                .or_insert_with(|| {
                                    AccumulatedAttribute::new(
                                        attr.name.clone(),
                                        attr.r#type.clone(),
                                    )
                                });
                            attr_entry.add_example(&attr.value);
                        }
                    }
                }
                DataPoints::Histogram(points) => {
                    for point in points {
                        for attr in point.attributes {
                            let attr_entry = entry
                                .attributes
                                .entry(attr.name.clone())
                                .or_insert_with(|| {
                                    AccumulatedAttribute::new(
                                        attr.name.clone(),
                                        attr.r#type.clone(),
                                    )
                                });
                            attr_entry.add_example(&attr.value);
                        }
                    }
                }
                DataPoints::ExponentialHistogram(points) => {
                    for point in points {
                        for attr in point.attributes {
                            let attr_entry = entry
                                .attributes
                                .entry(attr.name.clone())
                                .or_insert_with(|| {
                                    AccumulatedAttribute::new(
                                        attr.name.clone(),
                                        attr.r#type.clone(),
                                    )
                                });
                            attr_entry.add_example(&attr.value);
                        }
                    }
                }
            }
        }
    }

    fn add_event(&mut self, event_name: String, attributes: Vec<SampleAttribute>) {
        if event_name.is_empty() {
            return;
        }

        let entry = self
            .events
            .entry(event_name.clone())
            .or_insert_with(|| AccumulatedEvent::new(event_name));

        for attr in attributes {
            let attr_entry = entry
                .attributes
                .entry(attr.name.clone())
                .or_insert_with(|| {
                    AccumulatedAttribute::new(attr.name.clone(), attr.r#type.clone())
                });
            attr_entry.add_example(&attr.value);
        }
    }

    fn is_empty(&self) -> bool {
        self.resources.is_empty()
            && self.spans.is_empty()
            && self.metrics.is_empty()
            && self.events.is_empty()
    }

    fn stats(&self) -> (usize, usize, usize, usize) {
        (
            self.resources.len(),
            self.spans.len(),
            self.metrics.len(),
            self.events.len(),
        )
    }

    fn to_registry_file(&self) -> RegistryFile {
        let mut groups = Vec::new();

        // Resource group
        // Note: OTLP supports EntityRef (currently in Development status) which allows
        // grouping resource attributes by entity type (e.g., "service", "host").
        // We don't support entities yet, so all resource attributes are accumulated
        // into a single resource group.
        if !self.resources.is_empty() {
            let mut attributes: Vec<YamlAttribute> = self
                .resources
                .values()
                .map(|attr| YamlAttribute {
                    id: attr.name.clone(),
                    r#type: type_to_string(&attr.attr_type),
                    brief: String::new(),
                    examples: attr.examples.clone(),
                })
                .collect();
            attributes.sort_by(|a, b| a.id.cmp(&b.id));

            groups.push(YamlGroup {
                id: "resource".to_string(),
                r#type: "resource".to_string(),
                brief: String::new(),
                span_kind: None,
                metric_name: None,
                instrument: None,
                unit: None,
                name: None,
                attributes,
            });
        }

        // Span groups
        for span in self.spans.values() {
            let mut attributes: Vec<YamlAttribute> = span
                .attributes
                .values()
                .map(|attr| YamlAttribute {
                    id: attr.name.clone(),
                    r#type: type_to_string(&attr.attr_type),
                    brief: String::new(),
                    examples: attr.examples.clone(),
                })
                .collect();
            attributes.sort_by(|a, b| a.id.cmp(&b.id));

            groups.push(YamlGroup {
                id: format!("span.{}", sanitize_id(&span.name)),
                r#type: "span".to_string(),
                brief: String::new(),
                span_kind: Some(span_kind_to_string(&span.kind)),
                metric_name: None,
                instrument: None,
                unit: None,
                name: None,
                attributes,
            });

            // Span events as separate event groups
            for event in span.events.values() {
                let mut event_attributes: Vec<YamlAttribute> = event
                    .attributes
                    .values()
                    .map(|attr| YamlAttribute {
                        id: attr.name.clone(),
                        r#type: type_to_string(&attr.attr_type),
                        brief: String::new(),
                        examples: attr.examples.clone(),
                    })
                    .collect();
                event_attributes.sort_by(|a, b| a.id.cmp(&b.id));

                groups.push(YamlGroup {
                    id: format!("span_event.{}", sanitize_id(&event.name)),
                    r#type: "event".to_string(),
                    brief: String::new(),
                    span_kind: None,
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: Some(event.name.clone()),
                    attributes: event_attributes,
                });
            }
        }

        // Metric groups
        for metric in self.metrics.values() {
            let mut attributes: Vec<YamlAttribute> = metric
                .attributes
                .values()
                .map(|attr| YamlAttribute {
                    id: attr.name.clone(),
                    r#type: type_to_string(&attr.attr_type),
                    brief: String::new(),
                    examples: attr.examples.clone(),
                })
                .collect();
            attributes.sort_by(|a, b| a.id.cmp(&b.id));

            groups.push(YamlGroup {
                id: format!("metric.{}", sanitize_id(&metric.name)),
                r#type: "metric".to_string(),
                brief: String::new(),
                span_kind: None,
                metric_name: Some(metric.name.clone()),
                instrument: instrument_to_string(&metric.instrument),
                unit: if metric.unit.is_empty() {
                    None
                } else {
                    Some(metric.unit.clone())
                },
                name: None,
                attributes,
            });
        }

        // Event groups (from logs)
        for event in self.events.values() {
            let mut attributes: Vec<YamlAttribute> = event
                .attributes
                .values()
                .map(|attr| YamlAttribute {
                    id: attr.name.clone(),
                    r#type: type_to_string(&attr.attr_type),
                    brief: String::new(),
                    examples: attr.examples.clone(),
                })
                .collect();
            attributes.sort_by(|a, b| a.id.cmp(&b.id));

            groups.push(YamlGroup {
                id: format!("event.{}", sanitize_id(&event.name)),
                r#type: "event".to_string(),
                brief: String::new(),
                span_kind: None,
                metric_name: None,
                instrument: None,
                unit: None,
                name: Some(event.name.clone()),
                attributes,
            });
        }

        RegistryFile { groups }
    }
}

#[derive(Serialize)]
struct RegistryFile {
    groups: Vec<YamlGroup>,
}

#[derive(Serialize)]
struct YamlGroup {
    id: String,
    r#type: String,
    brief: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    span_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metric_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instrument: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attributes: Vec<YamlAttribute>,
}

#[derive(Serialize)]
struct YamlAttribute {
    id: String,
    r#type: String,
    brief: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    examples: Vec<Value>,
}

fn type_to_string(t: &Option<PrimitiveOrArrayTypeSpec>) -> String {
    match t {
        Some(PrimitiveOrArrayTypeSpec::Boolean) => "boolean".to_string(),
        Some(PrimitiveOrArrayTypeSpec::Int) => "int".to_string(),
        Some(PrimitiveOrArrayTypeSpec::Double) => "double".to_string(),
        Some(PrimitiveOrArrayTypeSpec::String) => "string".to_string(),
        Some(PrimitiveOrArrayTypeSpec::Booleans) => "boolean[]".to_string(),
        Some(PrimitiveOrArrayTypeSpec::Ints) => "int[]".to_string(),
        Some(PrimitiveOrArrayTypeSpec::Doubles) => "double[]".to_string(),
        Some(PrimitiveOrArrayTypeSpec::Strings) => "string[]".to_string(),
        Some(PrimitiveOrArrayTypeSpec::Any) | None => "string".to_string(),
    }
}

fn span_kind_to_string(kind: &SpanKindSpec) -> String {
    match kind {
        SpanKindSpec::Internal => "internal".to_string(),
        SpanKindSpec::Client => "client".to_string(),
        SpanKindSpec::Server => "server".to_string(),
        SpanKindSpec::Producer => "producer".to_string(),
        SpanKindSpec::Consumer => "consumer".to_string(),
    }
}

fn instrument_to_string(instrument: &Option<InstrumentSpec>) -> Option<String> {
    instrument.as_ref().map(|i| match i {
        InstrumentSpec::Counter => "counter".to_string(),
        InstrumentSpec::UpDownCounter => "updowncounter".to_string(),
        InstrumentSpec::Gauge => "gauge".to_string(),
        InstrumentSpec::Histogram => "histogram".to_string(),
    })
}

fn sanitize_id(name: &str) -> String {
    name.replace(['/', ' ', '-', '.'], "_")
        .to_lowercase()
        .trim_matches('_')
        .to_string()
}

fn process_otlp_request(request: OtlpRequest, accumulator: &mut AccumulatedSamples) -> bool {
    match request {
        OtlpRequest::Logs(logs) => {
            for resource_log in logs.resource_logs {
                if let Some(resource) = resource_log.resource {
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
                if let Some(resource) = resource_metric.resource {
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
                if let Some(resource) = resource_span.resource {
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
    info!("Weaver Registry Infer");
    info!(
        "Starting OTLP gRPC server on {}:{}",
        args.grpc_address, args.grpc_port
    );

    // Start the OTLP gRPC server and get an iterator of requests
    let requests = listen_otlp_requests(
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
        let registry = accumulator.to_registry_file();
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
