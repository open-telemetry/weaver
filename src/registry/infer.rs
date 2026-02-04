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

struct AccumulatedSpan {
    name: String,
    kind: SpanKindSpec,
    attributes: HashMap<String, AttributeSpec>,
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

struct AccumulatedMetric {
    name: String,
    instrument: InstrumentSpec,
    unit: String,
    attributes: HashMap<String, AttributeSpec>,
}

impl AccumulatedMetric {
    fn new(name: String, instrument: InstrumentSpec, unit: String) -> Self {
        Self {
            name,
            instrument,
            unit,
            attributes: HashMap::new(),
        }
    }
}

struct AccumulatedEvent {
    name: String,
    attributes: HashMap<String, AttributeSpec>,
}

impl AccumulatedEvent {
    fn new(name: String) -> Self {
        Self {
            name,
            attributes: HashMap::new(),
        }
    }
}

#[derive(Default)]
struct AccumulatedSamples {
    resources: HashMap<String, AttributeSpec>,
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
            Sample::Attribute(attr) => accumulate_attribute(&mut self.resources, attr),
            other => {
                // This shouldn't happen since we control when add_sample is called.
                // Adding anyway just in case.
                info!("Ignoring sample type {:?} - not yet supported", other);
            }
        }
    }

    fn add_resource(&mut self, resource: SampleResource) {
        for attr in resource.attributes {
            accumulate_attribute(&mut self.resources, attr);
        }
    }

    fn add_span(&mut self, span: SampleSpan) {
        let entry = self
            .spans
            .entry(span.name.clone())
            .or_insert_with(|| AccumulatedSpan::new(span.name.clone(), span.kind.clone()));

        for attr in span.attributes {
            accumulate_attribute(&mut entry.attributes, attr);
        }

        // TODO: Span events are being deprecated in the future. Eventually we should remove this.
        for event in span.span_events {
            let event_entry = entry
                .events
                .entry(event.name.clone())
                .or_insert_with(|| AccumulatedEvent::new(event.name.clone()));

            for attr in event.attributes {
                accumulate_attribute(&mut event_entry.attributes, attr);
            }
        }
    }

    fn add_metric(&mut self, metric: SampleMetric) {
        // Skip unsupported instrument types (e.g., Summary, Unspecified) - we can't infer a schema for them
        let instrument = match &metric.instrument {
            SampleInstrument::Supported(i) => i.clone(),
            SampleInstrument::Unsupported(_) => return,
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
                            accumulate_attribute(&mut entry.attributes, attr);
                        }
                    }
                }
                DataPoints::Histogram(points) => {
                    for point in points {
                        for attr in point.attributes {
                            accumulate_attribute(&mut entry.attributes, attr);
                        }
                    }
                }
                DataPoints::ExponentialHistogram(points) => {
                    for point in points {
                        for attr in point.attributes {
                            accumulate_attribute(&mut entry.attributes, attr);
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
            accumulate_attribute(&mut entry.attributes, attr);
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
            let mut attributes: Vec<AttributeSpec> = self.resources.values().cloned().collect();
            attributes.sort_by_key(|a| a.id());

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
            let mut attributes: Vec<AttributeSpec> = span.attributes.values().cloned().collect();
            attributes.sort_by_key(|a| a.id());

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
                let mut event_attributes: Vec<AttributeSpec> =
                    event.attributes.values().cloned().collect();
                event_attributes.sort_by_key(|a| a.id());

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
            let mut attributes: Vec<AttributeSpec> = metric.attributes.values().cloned().collect();
            attributes.sort_by_key(|a| a.id());

            groups.push(GroupSpec {
                id: format!("metric.{}", sanitize_id(&metric.name)),
                r#type: GroupType::Metric,
                brief: String::new(),
                stability: Some(Stability::Development),
                metric_name: Some(metric.name.clone()),
                instrument: Some(metric.instrument.clone()),
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
            let mut attributes: Vec<AttributeSpec> = event.attributes.values().cloned().collect();
            attributes.sort_by_key(|a| a.id());

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

/// Create a new AttributeSpec from a SampleAttribute.
fn attribute_spec_from_sample(sample: &SampleAttribute) -> AttributeSpec {
    let attr_type = sample
        .r#type
        .clone()
        .unwrap_or(PrimitiveOrArrayTypeSpec::String);

    let examples = sample.value.as_ref().and_then(|v| add_example(None, v));

    AttributeSpec::Id {
        id: sample.name.clone(),
        r#type: AttributeType::PrimitiveOrArray(attr_type),
        brief: Some(String::new()),
        examples,
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

/// Update an AttributeSpec's examples with a new value.
fn update_attribute_example(attr: &mut AttributeSpec, value: &Option<Value>) {
    if let Some(v) = value {
        if let AttributeSpec::Id { examples, .. } = attr {
            *examples = add_example(examples.take(), v);
        }
    }
}

/// Get or create an attribute in a HashMap, updating examples if it exists.
fn accumulate_attribute(attributes: &mut HashMap<String, AttributeSpec>, sample: SampleAttribute) {
    match attributes.entry(sample.name.clone()) {
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            update_attribute_example(entry.get_mut(), &sample.value);
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            let _ = entry.insert(attribute_spec_from_sample(&sample));
        }
    }
}

/// Add an example value to an existing Examples, handling type promotion and deduplication.
///
/// This function accumulates examples directly into the `Examples` type:
/// - If `current` is `None`, creates a new single-value `Examples`
/// - If `current` is a single value and new value is same type, promotes to array variant
/// - If `current` is already an array, appends while deduplicating
/// - On type mismatch, returns `current` unchanged
fn add_example(current: Option<Examples>, value: &Value) -> Option<Examples> {
    use weaver_common::ordered_float::OrderedF64;

    // Null values don't add anything
    if value.is_null() {
        return current;
    }

    match current {
        None => {
            // Create new single-value Examples from the JSON value
            match value {
                Value::Bool(b) => Some(Examples::Bool(*b)),
                Value::Number(n) => n
                    .as_i64()
                    .map(Examples::Int)
                    .or_else(|| n.as_f64().map(|f| Examples::Double(OrderedF64(f)))),
                Value::String(s) => Some(Examples::String(s.clone())),
                // Objects and arrays are not supported as example values
                Value::Array(_) | Value::Object(_) | Value::Null => None,
            }
        }
        Some(examples) => Some(add_to_existing_examples(examples, value)),
    }
}

/// Add a value to existing Examples, handling promotion from single to array.
fn add_to_existing_examples(examples: Examples, value: &Value) -> Examples {
    use weaver_common::ordered_float::OrderedF64;

    match (examples, value) {
        // Bool: single -> array promotion
        (Examples::Bool(existing), Value::Bool(new)) => {
            if existing == *new {
                Examples::Bool(existing)
            } else {
                Examples::Bools(vec![existing, *new])
            }
        }
        // Bool array: append
        (Examples::Bools(mut vec), Value::Bool(new)) => {
            if vec.len() < MAX_EXAMPLES && !vec.contains(new) {
                vec.push(*new);
            }
            Examples::Bools(vec)
        }

        // Int: single -> array promotion
        (Examples::Int(existing), Value::Number(n)) => match n.as_i64() {
            Some(new) if existing == new => Examples::Int(existing),
            Some(new) => Examples::Ints(vec![existing, new]),
            None => Examples::Int(existing),
        },
        // Int array: append
        (Examples::Ints(mut vec), Value::Number(n)) => {
            if let Some(new) = n.as_i64() {
                if vec.len() < MAX_EXAMPLES && !vec.contains(&new) {
                    vec.push(new);
                }
            }
            Examples::Ints(vec)
        }

        // Double: single -> array promotion
        (Examples::Double(existing), Value::Number(n)) => match n.as_f64() {
            Some(f) if existing == OrderedF64(f) => Examples::Double(existing),
            Some(f) => Examples::Doubles(vec![existing, OrderedF64(f)]),
            None => Examples::Double(existing),
        },
        // Double array: append
        (Examples::Doubles(mut vec), Value::Number(n)) => {
            if let Some(f) = n.as_f64() {
                let new = OrderedF64(f);
                if vec.len() < MAX_EXAMPLES && !vec.contains(&new) {
                    vec.push(new);
                }
            }
            Examples::Doubles(vec)
        }

        // String: single -> array promotion
        (Examples::String(existing), Value::String(new)) => {
            if existing == *new {
                Examples::String(existing)
            } else {
                Examples::Strings(vec![existing, new.clone()])
            }
        }
        // String array: append
        (Examples::Strings(mut vec), Value::String(new)) => {
            if vec.len() < MAX_EXAMPLES && !vec.contains(new) {
                vec.push(new.clone());
            }
            Examples::Strings(vec)
        }

        // Type mismatch or unsupported: return unchanged
        (examples, _) => examples,
    }
}

fn sanitize_id(name: &str) -> String {
    use convert_case::{Case, Casing};
    // Split by dots first (namespace separator), then apply snake_case to each segment
    name.split('.')
        .map(|segment| {
            // Replace other separators with spaces so Case::Snake can handle them
            let segment = segment.replace(['/', '-'], " ");
            segment.to_case(Case::Snake)
        })
        .collect::<Vec<_>>()
        .join(".")
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
    // Tests for add_example()
    // ============================================

    #[test]
    fn test_add_example_null_value_returns_current() {
        // Starting with None
        let result = add_example(None, &Value::Null);
        assert!(result.is_none());

        // Starting with existing examples
        let current = Some(Examples::String("existing".to_owned()));
        let result = add_example(current.clone(), &Value::Null);
        assert_eq!(result, current);
    }

    #[test]
    fn test_add_example_first_string_creates_single() {
        let result = add_example(None, &json!("hello"));
        assert_eq!(result, Some(Examples::String("hello".to_owned())));
    }

    #[test]
    fn test_add_example_first_int_creates_single() {
        let result = add_example(None, &json!(42));
        assert_eq!(result, Some(Examples::Int(42)));
    }

    #[test]
    fn test_add_example_first_double_creates_single() {
        use weaver_common::ordered_float::OrderedF64;

        let result = add_example(None, &json!(3.14));
        assert_eq!(result, Some(Examples::Double(OrderedF64(3.14))));
    }

    #[test]
    fn test_add_example_first_bool_creates_single() {
        let result = add_example(None, &json!(true));
        assert_eq!(result, Some(Examples::Bool(true)));
    }

    #[test]
    fn test_add_example_second_string_creates_array() {
        let current = Some(Examples::String("first".to_owned()));
        let result = add_example(current, &json!("second"));
        assert_eq!(
            result,
            Some(Examples::Strings(vec![
                "first".to_owned(),
                "second".to_owned()
            ]))
        );
    }

    #[test]
    fn test_add_example_deduplicates_in_array() {
        let current = Some(Examples::Strings(vec!["a".to_owned(), "b".to_owned()]));
        let result = add_example(current, &json!("a"));
        // Should not add duplicate
        assert_eq!(
            result,
            Some(Examples::Strings(vec!["a".to_owned(), "b".to_owned()]))
        );
    }

    #[test]
    fn test_add_example_respects_max_limit() {
        // MAX_EXAMPLES is 5, so start with 5 and try to add a 6th
        let current = Some(Examples::Ints(vec![1, 2, 3, 4, 5]));
        let result = add_example(current.clone(), &json!(6));
        // Should not add beyond max
        assert_eq!(result, current);
    }

    #[test]
    fn test_add_example_type_mismatch_returns_current() {
        // String examples, trying to add int
        let current = Some(Examples::String("text".to_owned()));
        let result = add_example(current.clone(), &json!(42));
        assert_eq!(result, current);

        // Int examples, trying to add string
        let current = Some(Examples::Int(42));
        let result = add_example(current.clone(), &json!("text"));
        assert_eq!(result, current);
    }

    #[test]
    fn test_add_example_type_mismatch_array_returns_current() {
        let current = Some(Examples::Strings(vec!["a".to_owned()]));
        let result = add_example(current.clone(), &json!(123));
        assert_eq!(result, current);
    }

    // ============================================
    // Tests for sanitize_id()
    // ============================================
    #[test]
    fn test_sanitize_id_replaces_special_characters() {
        assert_eq!(sanitize_id("http/request"), "http_request");
        assert_eq!(sanitize_id("http request"), "http_request");
        assert_eq!(sanitize_id("http-request"), "http_request");
        assert_eq!(sanitize_id("http.request"), "http.request");
    }

    #[test]
    fn test_sanitize_id_converts_to_lowercase() {
        assert_eq!(sanitize_id("HTTP_REQUEST"), "http_request");
        // Case::Snake properly splits camelCase
        assert_eq!(sanitize_id("HttpRequest"), "http_request");
    }

    #[test]
    fn test_sanitize_id_preserves_dots_as_namespace_separator() {
        // Dots are preserved as namespace separators
        assert_eq!(sanitize_id("http.server.duration"), "http.server.duration");
        // Each segment is snake_cased independently
        assert_eq!(
            sanitize_id("HttpServer.RequestDuration"),
            "http_server.request_duration"
        );
    }

    // ============================================
    // Tests for attribute_spec_from_sample()
    // ============================================

    #[test]
    fn test_attribute_spec_from_sample_creates_correct_spec() {
        let sample = SampleAttribute {
            name: "test.attr".to_owned(),
            r#type: Some(PrimitiveOrArrayTypeSpec::String),
            value: Some(json!("example")),
            live_check_result: None,
        };

        let spec = attribute_spec_from_sample(&sample);

        match spec {
            AttributeSpec::Id {
                id,
                r#type,
                examples,
                stability,
                ..
            } => {
                assert_eq!(id, "test.attr");
                assert_eq!(
                    r#type,
                    AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String)
                );
                assert_eq!(examples, Some(Examples::String("example".to_owned())));
                assert_eq!(stability, Some(Stability::Development));
            }
            AttributeSpec::Ref { .. } => panic!("Expected AttributeSpec::Id"),
        }
    }

    #[test]
    fn test_attribute_spec_from_sample_no_examples_when_no_value() {
        let sample = SampleAttribute {
            name: "test.attr".to_owned(),
            r#type: Some(PrimitiveOrArrayTypeSpec::Int),
            value: None,
            live_check_result: None,
        };

        let spec = attribute_spec_from_sample(&sample);

        match spec {
            AttributeSpec::Id { examples, .. } => {
                assert!(examples.is_none());
            }
            AttributeSpec::Ref { .. } => panic!("Expected AttributeSpec::Id"),
        }
    }

    // ============================================
    // Tests for accumulate_attribute()
    // ============================================

    #[test]
    fn test_accumulate_attribute_creates_new() {
        let mut attributes: HashMap<String, AttributeSpec> = HashMap::new();

        let sample = SampleAttribute {
            name: "test.attr".to_owned(),
            r#type: Some(PrimitiveOrArrayTypeSpec::String),
            value: Some(json!("value1")),
            live_check_result: None,
        };

        accumulate_attribute(&mut attributes, sample);

        assert_eq!(attributes.len(), 1);
        assert!(attributes.contains_key("test.attr"));
    }

    #[test]
    fn test_accumulate_attribute_updates_examples() {
        let mut attributes: HashMap<String, AttributeSpec> = HashMap::new();

        // Add first sample
        accumulate_attribute(
            &mut attributes,
            SampleAttribute {
                name: "test.attr".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("value1")),
                live_check_result: None,
            },
        );

        // Add second sample with same name
        accumulate_attribute(
            &mut attributes,
            SampleAttribute {
                name: "test.attr".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("value2")),
                live_check_result: None,
            },
        );

        // Should still have only 1 attribute
        assert_eq!(attributes.len(), 1);

        // But examples should be updated
        let attr = attributes.get("test.attr").unwrap();
        match attr {
            AttributeSpec::Id { examples, .. } => {
                assert_eq!(
                    *examples,
                    Some(Examples::Strings(vec![
                        "value1".to_owned(),
                        "value2".to_owned()
                    ]))
                );
            }
            AttributeSpec::Ref { .. } => panic!("Expected AttributeSpec::Id"),
        }
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
    fn test_accumulated_samples_add_resource() {
        let mut acc = AccumulatedSamples::new();

        let resource = SampleResource {
            attributes: vec![SampleAttribute {
                name: "service.name".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("my-service")),
                live_check_result: None,
            }],
            live_check_result: None,
        };

        acc.add_resource(resource);

        assert!(!acc.is_empty());
        assert_eq!(acc.stats(), (1, 0, 0, 0));
        assert!(acc.resources.contains_key("service.name"));

        let attr_spec = acc.resources.get("service.name").unwrap();
        match attr_spec {
            AttributeSpec::Id { examples, .. } => {
                assert_eq!(*examples, Some(Examples::String("my-service".to_owned())));
            }
            AttributeSpec::Ref { .. } => panic!("Expected AttributeSpec::Id"),
        }
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

        accumulate_attribute(
            &mut acc.resources,
            SampleAttribute {
                name: "service.name".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("test-service")),
                live_check_result: None,
            },
        );

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
        assert_eq!(group.id, "metric.http.server.duration");
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
        assert_eq!(group.id, "event.user.signup");
        assert_eq!(group.r#type, GroupType::Event);
        assert_eq!(group.name, Some("user.signup".to_owned()));
    }

    #[test]
    fn test_to_semconv_spec_attributes_are_sorted() {
        let mut acc = AccumulatedSamples::new();

        // Add attributes in non-alphabetical order
        accumulate_attribute(
            &mut acc.resources,
            SampleAttribute {
                name: "z.attr".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: None,
                live_check_result: None,
            },
        );
        accumulate_attribute(
            &mut acc.resources,
            SampleAttribute {
                name: "a.attr".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: None,
                live_check_result: None,
            },
        );
        accumulate_attribute(
            &mut acc.resources,
            SampleAttribute {
                name: "m.attr".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: None,
                live_check_result: None,
            },
        );

        let registry = acc.to_semconv_spec();

        let group = &registry.groups[0];
        let attr_ids: Vec<_> = group.attributes.iter().map(|a| a.id()).collect();
        assert_eq!(attr_ids, vec!["a.attr", "m.attr", "z.attr"]);
    }
}
