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
use weaver_semconv::attribute::{
    AttributeSpec, AttributeType, Examples, PrimitiveOrArrayTypeSpec, RequirementLevel,
};
use weaver_semconv::group::{GroupSpec, GroupType, InstrumentSpec, SpanKindSpec};
use weaver_semconv::stability::Stability;

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

        // TODO: Span events are being deprecated in the future. Eventually we should remove this.
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

    /// Convert accumulated samples to a semconv-compatible registry file.
    ///
    /// This method produces `GroupSpec` instances from `weaver_semconv` which
    /// ensures the output YAML follows the official semantic convention schema.
    fn to_semconv_spec(&self) -> InferredRegistry {
        let mut groups = Vec::new();

        // Resource group
        // Note: OTLP supports EntityRef (currently in Development status) which allows
        // grouping resource attributes by entity type (e.g., "service", "host").
        // We don't support entities yet, so all resource attributes are accumulated
        // into a single resource group.
        if !self.resources.is_empty() {
            let mut attributes: Vec<AttributeSpec> = self
                .resources
                .values()
                .map(accumulated_to_attribute_spec)
                .collect();
            attributes.sort_by(|a, b| a.id().cmp(&b.id()));

            groups.push(GroupSpec {
                id: "resource".to_owned(),
                r#type: GroupType::Entity,
                brief: String::new(),
                stability: Some(Stability::Development),
                attributes,
                ..Default::default()
            });
        }

        // Span groups
        for span in self.spans.values() {
            let mut attributes: Vec<AttributeSpec> = span
                .attributes
                .values()
                .map(accumulated_to_attribute_spec)
                .collect();
            attributes.sort_by(|a, b| a.id().cmp(&b.id()));

            groups.push(GroupSpec {
                id: format!("span.{}", sanitize_id(&span.name)),
                r#type: GroupType::Span,
                brief: String::new(),
                stability: Some(Stability::Development),
                span_kind: Some(span.kind.clone()),
                attributes,
                ..Default::default()
            });

            // Span events as separate event groups
            for event in span.events.values() {
                let mut event_attributes: Vec<AttributeSpec> = event
                    .attributes
                    .values()
                    .map(accumulated_to_attribute_spec)
                    .collect();
                event_attributes.sort_by(|a, b| a.id().cmp(&b.id()));

                groups.push(GroupSpec {
                    id: format!("span_event.{}", sanitize_id(&event.name)),
                    r#type: GroupType::Event,
                    brief: String::new(),
                    stability: Some(Stability::Development),
                    name: Some(event.name.clone()),
                    attributes: event_attributes,
                    ..Default::default()
                });
            }
        }

        // Metric groups
        for metric in self.metrics.values() {
            let mut attributes: Vec<AttributeSpec> = metric
                .attributes
                .values()
                .map(accumulated_to_attribute_spec)
                .collect();
            attributes.sort_by(|a, b| a.id().cmp(&b.id()));

            groups.push(GroupSpec {
                id: format!("metric.{}", sanitize_id(&metric.name)),
                r#type: GroupType::Metric,
                brief: String::new(),
                stability: Some(Stability::Development),
                metric_name: Some(metric.name.clone()),
                instrument: metric.instrument.clone(),
                unit: if metric.unit.is_empty() {
                    None
                } else {
                    Some(metric.unit.clone())
                },
                attributes,
                ..Default::default()
            });
        }

        // Event groups (from logs)
        for event in self.events.values() {
            let mut attributes: Vec<AttributeSpec> = event
                .attributes
                .values()
                .map(accumulated_to_attribute_spec)
                .collect();
            attributes.sort_by(|a, b| a.id().cmp(&b.id()));

            groups.push(GroupSpec {
                id: format!("event.{}", sanitize_id(&event.name)),
                r#type: GroupType::Event,
                brief: String::new(),
                stability: Some(Stability::Development),
                name: Some(event.name.clone()),
                attributes,
                ..Default::default()
            });
        }

        InferredRegistry { groups }
    }
}

/// Wrapper for serializing a list of GroupSpec as a semconv registry file.
///
/// Note: We use this wrapper instead of `SemConvSpecV1` directly because
/// `SemConvSpecV1.groups` is `pub(crate)` in weaver_semconv.
/// This wrapper produces the same YAML structure as a valid semconv file.
#[derive(Serialize)]
struct InferredRegistry {
    groups: Vec<GroupSpec>,
}

/// Convert an accumulated attribute to a weaver_semconv AttributeSpec.
fn accumulated_to_attribute_spec(attr: &AccumulatedAttribute) -> AttributeSpec {
    let attr_type = attr
        .attr_type
        .clone()
        .unwrap_or(PrimitiveOrArrayTypeSpec::String);

    AttributeSpec::Id {
        id: attr.name.clone(),
        r#type: AttributeType::PrimitiveOrArray(attr_type),
        brief: Some(String::new()),
        examples: json_values_to_examples(&attr.examples),
        tag: None,
        requirement_level: RequirementLevel::default(),
        sampling_relevant: None,
        note: String::new(),
        stability: Some(Stability::Development),
        deprecated: None,
        annotations: None,
        role: None,
    }
}

/// Convert a vector of JSON values to the appropriate Examples type.
///
/// Uses serde to automatically match the JSON values to the correct Examples variant.
fn json_values_to_examples(values: &[Value]) -> Option<Examples> {
    if values.is_empty() {
        return None;
    }

    // Try to convert: if single value, try as single example; if multiple, try as array
    if values.len() == 1 {
        serde_json::from_value::<Examples>(values[0].clone()).ok()
    } else {
        // For multiple values, create an array and let serde figure out the type
        let arr = Value::Array(values.to_vec());
        serde_json::from_value::<Examples>(arr).ok()
    }
}

fn sanitize_id(name: &str) -> String {
    name.replace(['/', ' ', '-', '.'], "_")
        .to_lowercase()
        .trim_matches('_')
        .to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use weaver_semconv::group::SpanKindSpec;

    // ============================================
    // Tests for sanitize_id()
    // ============================================
    #[test]
    fn test_sanitize_id_replaces_special_characters() {
        assert_eq!(sanitize_id("http/request"), "http_request");
        assert_eq!(sanitize_id("http request"), "http_request");
        assert_eq!(sanitize_id("http-request"), "http_request");
        assert_eq!(sanitize_id("http.request"), "http_request");
    }

    #[test]
    fn test_sanitize_id_converts_to_lowercase() {
        assert_eq!(sanitize_id("HTTP_REQUEST"), "http_request");
        assert_eq!(sanitize_id("HttpRequest"), "httprequest");
    }

    #[test]
    fn test_sanitize_id_trims_underscores() {
        assert_eq!(sanitize_id("_http_request_"), "http_request");
        assert_eq!(sanitize_id("/http/request/"), "http_request");
        assert_eq!(sanitize_id("...test..."), "test");
    }

    // ============================================
    // Tests for json_values_to_examples()
    // ============================================

    #[test]
    fn test_json_values_to_examples_empty() {
        let result = json_values_to_examples(&[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_json_values_to_examples_single_string() {
        let values = vec![json!("hello")];
        let result = json_values_to_examples(&values);
        assert!(result.is_some());
        // The Examples type should contain "hello"
        let examples = result.unwrap();
        assert_eq!(examples, Examples::String("hello".to_owned()));
    }

    #[test]
    fn test_json_values_to_examples_single_int() {
        let values = vec![json!(42)];
        let result = json_values_to_examples(&values);
        assert!(result.is_some());
        let examples = result.unwrap();
        assert_eq!(examples, Examples::Int(42));
    }

    #[test]
    fn test_json_values_to_examples_single_double() {
        use weaver_common::ordered_float::OrderedF64;

        let values = vec![json!(3.14)];
        let result = json_values_to_examples(&values);
        assert_eq!(result, Some(Examples::Double(OrderedF64(3.14))));
    }

    #[test]
    fn test_json_values_to_examples_single_bool() {
        let values = vec![json!(true)];
        let result = json_values_to_examples(&values);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Examples::Bool(true));
    }

    #[test]
    fn test_json_values_to_examples_multiple_strings() {
        let values = vec![json!("hello"), json!("world")];
        let result = json_values_to_examples(&values);
        assert!(result.is_some());
        let examples = result.unwrap();
        assert_eq!(
            examples,
            Examples::Strings(vec!["hello".to_owned(), "world".to_owned()])
        );
    }

    #[test]
    fn test_json_values_to_examples_multiple_ints() {
        let values = vec![json!(1), json!(2), json!(3)];
        let result = json_values_to_examples(&values);
        assert!(result.is_some());
        let examples = result.unwrap();
        assert_eq!(examples, Examples::Ints(vec![1, 2, 3]));
    }

    // ============================================
    // Tests for AccumulatedAttribute
    // ============================================

    #[test]
    fn test_accumulated_attribute_new() {
        let attr = AccumulatedAttribute::new(
            "test.attr".to_owned(),
            Some(PrimitiveOrArrayTypeSpec::String),
        );
        assert_eq!(attr.name, "test.attr");
        assert_eq!(attr.attr_type, Some(PrimitiveOrArrayTypeSpec::String));
        assert!(attr.examples.is_empty());
    }

    #[test]
    fn test_accumulated_attribute_add_example_respects_max() {
        let mut attr =
            AccumulatedAttribute::new("test".to_owned(), Some(PrimitiveOrArrayTypeSpec::Int));

        // Add MAX_EXAMPLES + 2 unique values
        for i in 0..(MAX_EXAMPLES + 2) {
            attr.add_example(&Some(json!(i)));
        }

        // Should only have MAX_EXAMPLES
        assert_eq!(attr.examples.len(), MAX_EXAMPLES);
        // First MAX_EXAMPLES values should be preserved
        for i in 0..MAX_EXAMPLES {
            assert!(attr.examples.contains(&json!(i)));
        }
    }

    #[test]
    fn test_accumulated_attribute_add_example_deduplicates() {
        let mut attr =
            AccumulatedAttribute::new("test".to_owned(), Some(PrimitiveOrArrayTypeSpec::String));

        // Add same value multiple times
        attr.add_example(&Some(json!("duplicate")));
        attr.add_example(&Some(json!("duplicate")));
        attr.add_example(&Some(json!("duplicate")));

        assert_eq!(attr.examples.len(), 1);
        assert_eq!(attr.examples[0], json!("duplicate"));
    }

    #[test]
    fn test_accumulated_attribute_add_example_ignores_none() {
        let mut attr =
            AccumulatedAttribute::new("test".to_owned(), Some(PrimitiveOrArrayTypeSpec::String));

        attr.add_example(&None);
        attr.add_example(&None);

        assert!(attr.examples.is_empty());
    }

    // ============================================
    // Tests for AccumulatedSamples
    // ============================================

    #[test]
    fn test_accumulated_samples_new_is_empty() {
        let acc = AccumulatedSamples::new();
        assert!(acc.is_empty());
        assert_eq!(acc.stats(), (0, 0, 0, 0));
    }

    #[test]
    fn test_accumulated_samples_add_resource_attribute() {
        let mut acc = AccumulatedSamples::new();

        let attr = SampleAttribute {
            name: "service.name".to_owned(),
            r#type: Some(PrimitiveOrArrayTypeSpec::String),
            value: Some(json!("my-service")),
            live_check_result: None,
        };

        acc.add_resource_attribute(attr);

        assert!(!acc.is_empty());
        assert_eq!(acc.stats(), (1, 0, 0, 0));
        assert!(acc.resources.contains_key("service.name"));

        let accumulated = acc.resources.get("service.name").unwrap();
        assert_eq!(accumulated.examples.len(), 1);
        assert_eq!(accumulated.examples[0], json!("my-service"));
    }

    #[test]
    fn test_accumulated_samples_add_span() {
        let mut acc = AccumulatedSamples::new();

        let span = SampleSpan {
            name: "GET /api/users".to_owned(),
            kind: SpanKindSpec::Server,
            status: None,
            attributes: vec![SampleAttribute {
                name: "http.method".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("GET")),
                live_check_result: None,
            }],
            span_events: vec![],
            span_links: vec![],
            live_check_result: None,
        };

        acc.add_span(span);

        assert_eq!(acc.stats(), (0, 1, 0, 0));
        assert!(acc.spans.contains_key("GET /api/users"));

        let accumulated_span = acc.spans.get("GET /api/users").unwrap();
        assert_eq!(accumulated_span.kind, SpanKindSpec::Server);
        assert!(accumulated_span.attributes.contains_key("http.method"));
    }

    #[test]
    fn test_accumulated_samples_add_span_with_events() {
        let mut acc = AccumulatedSamples::new();

        let span = SampleSpan {
            name: "process".to_owned(),
            kind: SpanKindSpec::Internal,
            status: None,
            attributes: vec![],
            span_events: vec![SampleSpanEvent {
                name: "exception".to_owned(),
                attributes: vec![SampleAttribute {
                    name: "exception.type".to_owned(),
                    r#type: Some(PrimitiveOrArrayTypeSpec::String),
                    value: Some(json!("NullPointerException")),
                    live_check_result: None,
                }],
                live_check_result: None,
            }],
            span_links: vec![],
            live_check_result: None,
        };

        acc.add_span(span);

        let accumulated_span = acc.spans.get("process").unwrap();
        assert!(accumulated_span.events.contains_key("exception"));

        let event = accumulated_span.events.get("exception").unwrap();
        assert!(event.attributes.contains_key("exception.type"));
    }

    #[test]
    fn test_accumulated_samples_add_event_ignores_empty_name() {
        let mut acc = AccumulatedSamples::new();

        acc.add_event(
            String::new(),
            vec![SampleAttribute {
                name: "attr".to_owned(),
                r#type: None,
                value: Some(json!("value")),
                live_check_result: None,
            }],
        );

        assert!(acc.events.is_empty());
    }

    #[test]
    fn test_accumulated_samples_add_event_with_name() {
        let mut acc = AccumulatedSamples::new();

        acc.add_event(
            "user.login".to_owned(),
            vec![SampleAttribute {
                name: "user.id".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("user-123")),
                live_check_result: None,
            }],
        );

        assert_eq!(acc.stats(), (0, 0, 0, 1));
        assert!(acc.events.contains_key("user.login"));
    }

    // ============================================
    // Tests for to_semconv_spec()
    // ============================================

    #[test]
    fn test_to_semconv_spec_empty_accumulator() {
        let acc = AccumulatedSamples::new();
        let registry = acc.to_semconv_spec();

        assert!(registry.groups.is_empty());
    }

    #[test]
    fn test_to_semconv_spec_with_resources() {
        let mut acc = AccumulatedSamples::new();

        acc.add_resource_attribute(SampleAttribute {
            name: "service.name".to_owned(),
            r#type: Some(PrimitiveOrArrayTypeSpec::String),
            value: Some(json!("test-service")),
            live_check_result: None,
        });

        let registry = acc.to_semconv_spec();

        assert_eq!(registry.groups.len(), 1);
        let group = &registry.groups[0];
        assert_eq!(group.id, "resource");
        assert_eq!(group.r#type, GroupType::Entity);
        assert_eq!(group.stability, Some(Stability::Development));
        assert_eq!(group.attributes.len(), 1);
    }

    #[test]
    fn test_to_semconv_spec_with_span() {
        let mut acc = AccumulatedSamples::new();

        acc.add_span(SampleSpan {
            name: "HTTP GET".to_owned(),
            kind: SpanKindSpec::Client,
            status: None,
            attributes: vec![SampleAttribute {
                name: "http.url".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("https://example.com")),
                live_check_result: None,
            }],
            span_events: vec![],
            span_links: vec![],
            live_check_result: None,
        });

        let registry = acc.to_semconv_spec();

        assert_eq!(registry.groups.len(), 1);
        let group = &registry.groups[0];
        assert_eq!(group.id, "span.http_get");
        assert_eq!(group.r#type, GroupType::Span);
        assert_eq!(group.span_kind, Some(SpanKindSpec::Client));
        assert_eq!(group.attributes.len(), 1);
    }

    #[test]
    fn test_to_semconv_spec_with_metric() {
        let mut acc = AccumulatedSamples::new();

        let metric = SampleMetric {
            name: "http.server.duration".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Histogram),
            unit: "ms".to_owned(),
            data_points: None,
            live_check_result: None,
        };

        acc.add_metric(metric);

        let registry = acc.to_semconv_spec();

        assert_eq!(registry.groups.len(), 1);
        let group = &registry.groups[0];
        assert_eq!(group.id, "metric.http_server_duration");
        assert_eq!(group.r#type, GroupType::Metric);
        assert_eq!(group.metric_name, Some("http.server.duration".to_owned()));
        assert_eq!(group.instrument, Some(InstrumentSpec::Histogram));
        assert_eq!(group.unit, Some("ms".to_owned()));
    }

    #[test]
    fn test_to_semconv_spec_metric_empty_unit_is_none() {
        let mut acc = AccumulatedSamples::new();

        let metric = SampleMetric {
            name: "custom.counter".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Counter),
            unit: String::new(), // Empty unit
            data_points: None,
            live_check_result: None,
        };

        acc.add_metric(metric);

        let registry = acc.to_semconv_spec();

        let group = &registry.groups[0];
        assert!(group.unit.is_none());
    }

    #[test]
    fn test_to_semconv_spec_with_event() {
        let mut acc = AccumulatedSamples::new();

        acc.add_event(
            "user.signup".to_owned(),
            vec![SampleAttribute {
                name: "user.email".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("test@example.com")),
                live_check_result: None,
            }],
        );

        let registry = acc.to_semconv_spec();

        assert_eq!(registry.groups.len(), 1);
        let group = &registry.groups[0];
        assert_eq!(group.id, "event.user_signup");
        assert_eq!(group.r#type, GroupType::Event);
        assert_eq!(group.name, Some("user.signup".to_owned()));
    }

    #[test]
    fn test_to_semconv_spec_attributes_are_sorted() {
        let mut acc = AccumulatedSamples::new();

        // Add attributes in non-alphabetical order
        acc.add_resource_attribute(SampleAttribute {
            name: "z.attr".to_owned(),
            r#type: Some(PrimitiveOrArrayTypeSpec::String),
            value: None,
            live_check_result: None,
        });
        acc.add_resource_attribute(SampleAttribute {
            name: "a.attr".to_owned(),
            r#type: Some(PrimitiveOrArrayTypeSpec::String),
            value: None,
            live_check_result: None,
        });
        acc.add_resource_attribute(SampleAttribute {
            name: "m.attr".to_owned(),
            r#type: Some(PrimitiveOrArrayTypeSpec::String),
            value: None,
            live_check_result: None,
        });

        let registry = acc.to_semconv_spec();

        let group = &registry.groups[0];
        let attr_ids: Vec<_> = group.attributes.iter().map(|a| a.id()).collect();
        assert_eq!(attr_ids, vec!["a.attr", "m.attr", "z.attr"]);
    }

    // ============================================
    // Tests for accumulated_to_attribute_spec()
    // ============================================

    #[test]
    fn test_accumulated_to_attribute_spec_defaults_to_string() {
        let attr = AccumulatedAttribute::new("test.attr".to_owned(), None);

        let spec = accumulated_to_attribute_spec(&attr);

        match spec {
            AttributeSpec::Id { r#type, .. } => {
                assert_eq!(
                    r#type,
                    AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String)
                );
            }
            _ => panic!("Expected AttributeSpec::Id"),
        }
    }

    #[test]
    fn test_accumulated_to_attribute_spec_preserves_type() {
        let attr =
            AccumulatedAttribute::new("test.attr".to_owned(), Some(PrimitiveOrArrayTypeSpec::Int));

        let spec = accumulated_to_attribute_spec(&attr);

        match spec {
            AttributeSpec::Id { r#type, .. } => {
                assert_eq!(
                    r#type,
                    AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int)
                );
            }
            _ => panic!("Expected AttributeSpec::Id"),
        }
    }

    #[test]
    fn test_accumulated_to_attribute_spec_includes_examples() {
        let mut attr = AccumulatedAttribute::new(
            "test.attr".to_owned(),
            Some(PrimitiveOrArrayTypeSpec::String),
        );
        attr.add_example(&Some(json!("example1")));
        attr.add_example(&Some(json!("example2")));

        let spec = accumulated_to_attribute_spec(&attr);

        match spec {
            AttributeSpec::Id { examples, .. } => {
                assert!(examples.is_some());
            }
            _ => panic!("Expected AttributeSpec::Id"),
        }
    }

    #[test]
    fn test_accumulated_to_attribute_spec_stability_is_development() {
        let attr = AccumulatedAttribute::new("test.attr".to_owned(), None);

        let spec = accumulated_to_attribute_spec(&attr);

        match spec {
            AttributeSpec::Id { stability, .. } => {
                assert_eq!(stability, Some(Stability::Development));
            }
            _ => panic!("Expected AttributeSpec::Id"),
        }
    }
}
