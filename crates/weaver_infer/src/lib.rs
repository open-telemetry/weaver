// SPDX-License-Identifier: Apache-2.0

//! Core inference logic for `weaver registry infer`.

use std::collections::{BTreeMap, HashMap};

use log::info;
use serde::Serialize;
use serde_json::Value;
use weaver_live_check::sample_attribute::SampleAttribute;
use weaver_live_check::sample_metric::{SampleInstrument, SampleMetric};
use weaver_live_check::sample_resource::SampleResource;
use weaver_live_check::sample_span::SampleSpan;
use weaver_live_check::Sample;
use weaver_semconv::attribute::{
    AttributeSpec, AttributeType, Examples, PrimitiveOrArrayTypeSpec, RequirementLevel,
};
use weaver_semconv::group::{InstrumentSpec, SpanKindSpec};
use weaver_semconv::stability::Stability;
use weaver_semconv::v2::{
    attribute::{AttributeDef, AttributeOrGroupRef, AttributeRef},
    entity::Entity,
    event::Event,
    metric::Metric,
    signal_id::SignalId,
    span::{Span, SpanAttributeOrGroupRef, SpanAttributeRef, SpanName},
    CommonFields,
};

const MAX_EXAMPLES: usize = 5;

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

/// Accumulates telemetry samples into inferred semantic convention groups.
#[derive(Default)]
pub struct AccumulatedSamples {
    resources: HashMap<String, AttributeSpec>,
    spans: HashMap<String, AccumulatedSpan>,
    metrics: HashMap<String, AccumulatedMetric>,
    events: HashMap<String, AccumulatedEvent>,
}

impl AccumulatedSamples {
    /// Creates a new empty accumulator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one telemetry sample to the accumulator.
    pub fn add_sample(&mut self, sample: Sample) {
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

    /// Returns true when no resource/span/metric/event was accumulated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
            && self.spans.is_empty()
            && self.metrics.is_empty()
            && self.events.is_empty()
    }

    /// Returns a tuple with counts: resource attrs, spans, metrics, events.
    #[must_use]
    pub fn stats(&self) -> (usize, usize, usize, usize) {
        (
            self.resources.len(),
            self.spans.len(),
            self.metrics.len(),
            self.events.len(),
        )
    }

    /// Converts accumulated samples to a v2 semconv-compatible registry file.
    #[must_use]
    pub fn to_semconv_spec(&self) -> InferredRegistryV2 {
        let mut attribute_defs = HashMap::new();
        collect_attribute_defs(self.resources.values(), &mut attribute_defs);

        let entities = if self.resources.is_empty() {
            vec![]
        } else {
            let mut identity = self
                .resources
                .keys()
                .map(|name| attribute_ref(name))
                .collect::<Vec<_>>();
            identity.sort_by(|left, right| left.r#ref.cmp(&right.r#ref));

            vec![Entity {
                r#type: SignalId::from("resource".to_owned()),
                identity,
                description: vec![],
                common: inferred_common_fields(),
            }]
        };

        let mut spans = self
            .spans
            .values()
            .map(|span| {
                collect_attribute_defs(span.attributes.values(), &mut attribute_defs);

                for event in span.events.values() {
                    collect_attribute_defs(event.attributes.values(), &mut attribute_defs);
                }

                let mut attributes = span
                    .attributes
                    .keys()
                    .map(|name| span_attribute_ref(name))
                    .collect::<Vec<_>>();
                attributes.sort_by(|left, right| {
                    span_attribute_ref_name(left).cmp(span_attribute_ref_name(right))
                });

                Span {
                    r#type: SignalId::from(span.name.clone()),
                    kind: span.kind.clone(),
                    name: SpanName {
                        note: span.name.clone(),
                    },
                    attributes,
                    entity_associations: vec![],
                    common: inferred_common_fields(),
                }
            })
            .collect::<Vec<_>>();
        spans.sort_by(|left, right| left.r#type.to_string().cmp(&right.r#type.to_string()));

        let mut metrics = self
            .metrics
            .values()
            .map(|metric| {
                collect_attribute_defs(metric.attributes.values(), &mut attribute_defs);

                let mut attributes = metric
                    .attributes
                    .keys()
                    .map(|name| attribute_or_group_ref(name))
                    .collect::<Vec<_>>();
                attributes.sort_by(|left, right| {
                    attribute_or_group_ref_name(left).cmp(attribute_or_group_ref_name(right))
                });

                Metric {
                    name: SignalId::from(metric.name.clone()),
                    instrument: metric.instrument.clone(),
                    unit: metric.unit.clone(),
                    attributes,
                    entity_associations: vec![],
                    common: inferred_common_fields(),
                }
            })
            .collect::<Vec<_>>();
        metrics.sort_by(|left, right| left.name.to_string().cmp(&right.name.to_string()));

        let mut merged_events: HashMap<String, Vec<String>> = self
            .events
            .values()
            .map(|event| {
                collect_attribute_defs(event.attributes.values(), &mut attribute_defs);
                (
                    event.name.clone(),
                    event.attributes.keys().cloned().collect::<Vec<_>>(),
                )
            })
            .collect();

        for span in self.spans.values() {
            for event in span.events.values() {
                collect_attribute_defs(event.attributes.values(), &mut attribute_defs);
                let merged_attributes = merged_events.entry(event.name.clone()).or_default();
                merged_attributes.extend(event.attributes.keys().cloned());
            }
        }

        let mut events = merged_events
            .into_iter()
            .map(|(name, mut attribute_names)| {
                attribute_names.sort();
                attribute_names.dedup();

                Event {
                    name: SignalId::from(name),
                    attributes: attribute_names
                        .into_iter()
                        .map(|attribute_name| attribute_or_group_ref(&attribute_name))
                        .collect(),
                    entity_associations: vec![],
                    common: inferred_common_fields(),
                }
            })
            .collect::<Vec<_>>();
        events.sort_by(|left, right| left.name.to_string().cmp(&right.name.to_string()));

        let mut attributes = attribute_defs.into_values().collect::<Vec<_>>();
        attributes.sort_by(|left, right| left.key.cmp(&right.key));

        InferredRegistryV2 {
            file_format: "definition/2",
            attributes,
            entities,
            events,
            metrics,
            spans,
        }
    }
}

/// Wrapper for serializing an inferred v2 semconv registry file.
#[derive(Serialize)]
pub struct InferredRegistryV2 {
    /// The semantic convention definition format.
    file_format: &'static str,
    /// Inferred semantic convention attributes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attributes: Vec<AttributeDef>,
    /// Inferred semantic convention entities.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    entities: Vec<Entity>,
    /// Inferred semantic convention spans.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    spans: Vec<Span>,
    /// Inferred semantic convention metrics.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    metrics: Vec<Metric>,
    /// Inferred semantic convention events.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    events: Vec<Event>,
}

/// Create a new `AttributeSpec` from a `SampleAttribute`.
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

/// Update an `AttributeSpec` examples field with a new value.
fn update_attribute_example(attr: &mut AttributeSpec, value: &Option<Value>) {
    if let Some(v) = value {
        if let AttributeSpec::Id { examples, .. } = attr {
            *examples = add_example(examples.take(), v);
        }
    }
}

/// Get or create an attribute in a map, updating examples if it already exists.
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

/// Add an example value to an existing `Examples`, with promotion and deduplication.
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

/// Add one value to existing `Examples`, handling single-to-array promotion.
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

#[cfg(test)]
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

fn inferred_common_fields() -> CommonFields {
    CommonFields {
        brief: String::new(),
        note: String::new(),
        stability: Stability::Development,
        deprecated: None,
        annotations: BTreeMap::new(),
    }
}

fn attribute_ref(name: &str) -> AttributeRef {
    AttributeRef {
        r#ref: name.to_owned(),
        brief: None,
        examples: None,
        requirement_level: None,
        note: None,
        stability: None,
        deprecated: None,
        annotations: BTreeMap::new(),
    }
}

fn span_attribute_ref(name: &str) -> SpanAttributeOrGroupRef {
    SpanAttributeOrGroupRef::Attribute(SpanAttributeRef {
        base: attribute_ref(name),
        sampling_relevant: None,
    })
}

fn attribute_or_group_ref(name: &str) -> AttributeOrGroupRef {
    AttributeOrGroupRef::Attribute(attribute_ref(name))
}

fn collect_attribute_defs<'a>(
    attributes: impl Iterator<Item = &'a AttributeSpec>,
    attribute_defs: &mut HashMap<String, AttributeDef>,
) {
    for attribute in attributes {
        if let AttributeSpec::Id {
            id,
            r#type,
            brief,
            examples,
            note,
            stability,
            deprecated,
            annotations,
            ..
        } = attribute
        {
            let _ = attribute_defs
                .entry(id.clone())
                .or_insert_with(|| AttributeDef {
                    key: id.clone(),
                    r#type: r#type.clone(),
                    examples: examples.clone(),
                    common: CommonFields {
                        brief: brief.clone().unwrap_or_default(),
                        note: note.clone(),
                        stability: stability.clone().unwrap_or(Stability::Development),
                        deprecated: deprecated.clone(),
                        annotations: annotations.clone().unwrap_or_default(),
                    },
                });
        }
    }
}

fn span_attribute_ref_name(attribute: &SpanAttributeOrGroupRef) -> &str {
    match attribute {
        SpanAttributeOrGroupRef::Attribute(attribute) => &attribute.base.r#ref,
        SpanAttributeOrGroupRef::Group(group) => &group.ref_group,
    }
}

fn attribute_or_group_ref_name(attribute: &AttributeOrGroupRef) -> &str {
    match attribute {
        AttributeOrGroupRef::Attribute(attribute) => &attribute.r#ref,
        AttributeOrGroupRef::Group(group) => &group.ref_group,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use weaver_live_check::sample_log::SampleLog;
    use weaver_live_check::sample_metric::{
        DataPoints, SampleExponentialHistogramDataPoint, SampleHistogramDataPoint,
        SampleNumberDataPoint,
    };
    use weaver_live_check::sample_span::SampleSpanEvent;
    use weaver_live_check::sample_span::SampleSpanLink;
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

        let result = add_example(None, &json!(1.5));
        assert_eq!(result, Some(Examples::Double(OrderedF64(1.5))));
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

    fn assert_add_example_cases(cases: &[(&str, Option<Examples>, Value, Option<Examples>)]) {
        for (name, current, value, expected) in cases {
            assert_eq!(
                add_example(current.clone(), value),
                expected.clone(),
                "case: {name}"
            );
        }
    }

    #[test]
    fn test_add_example_ignores_complex_values() {
        assert_add_example_cases(&[
            ("array values are ignored", None, json!([1, 2, 3]), None),
            (
                "object values are ignored",
                None,
                json!({"key": "value"}),
                None,
            ),
        ]);
    }

    #[test]
    fn test_add_example_bool_paths() {
        assert_add_example_cases(&[
            (
                "same bool stays scalar",
                Some(Examples::Bool(true)),
                json!(true),
                Some(Examples::Bool(true)),
            ),
            (
                "different bool promotes to array",
                Some(Examples::Bool(true)),
                json!(false),
                Some(Examples::Bools(vec![true, false])),
            ),
            (
                "bool array appends new value",
                Some(Examples::Bools(vec![true])),
                json!(false),
                Some(Examples::Bools(vec![true, false])),
            ),
        ]);
    }

    #[test]
    fn test_add_example_int_paths() {
        assert_add_example_cases(&[
            (
                "different int promotes to array",
                Some(Examples::Int(42)),
                json!(7),
                Some(Examples::Ints(vec![42, 7])),
            ),
            (
                "float does not update int example",
                Some(Examples::Int(42)),
                json!(1.5),
                Some(Examples::Int(42)),
            ),
            (
                "int array appends new value",
                Some(Examples::Ints(vec![1, 2])),
                json!(3),
                Some(Examples::Ints(vec![1, 2, 3])),
            ),
        ]);
    }

    #[test]
    fn test_add_example_double_paths() {
        use weaver_common::ordered_float::OrderedF64;

        assert_add_example_cases(&[
            (
                "same double stays scalar",
                Some(Examples::Double(OrderedF64(1.5))),
                json!(1.5),
                Some(Examples::Double(OrderedF64(1.5))),
            ),
            (
                "different double promotes to array",
                Some(Examples::Double(OrderedF64(1.5))),
                json!(2.5),
                Some(Examples::Doubles(vec![OrderedF64(1.5), OrderedF64(2.5)])),
            ),
            (
                "double array appends new value",
                Some(Examples::Doubles(vec![OrderedF64(1.5), OrderedF64(2.5)])),
                json!(3.5),
                Some(Examples::Doubles(vec![
                    OrderedF64(1.5),
                    OrderedF64(2.5),
                    OrderedF64(3.5),
                ])),
            ),
        ]);
    }

    #[test]
    fn test_add_example_string_paths() {
        assert_add_example_cases(&[
            (
                "same string stays scalar",
                Some(Examples::String("same".to_owned())),
                json!("same"),
                Some(Examples::String("same".to_owned())),
            ),
            (
                "string array appends new value",
                Some(Examples::Strings(vec!["a".to_owned()])),
                json!("b"),
                Some(Examples::Strings(vec!["a".to_owned(), "b".to_owned()])),
            ),
        ]);
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
        let attr = attributes.get("test.attr").expect("attribute should exist");
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

        let attr_spec = acc
            .resources
            .get("service.name")
            .expect("resource attribute should exist");
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
            resource: None,
        };

        acc.add_span(span);

        assert_eq!(acc.stats(), (0, 1, 0, 0));
        assert!(acc.spans.contains_key("GET /api/users"));

        let accumulated_span = acc.spans.get("GET /api/users").expect("span should exist");
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
            resource: None,
        };

        acc.add_span(span);

        let accumulated_span = acc.spans.get("process").expect("span should exist");
        assert!(accumulated_span.events.contains_key("exception"));

        let event = accumulated_span
            .events
            .get("exception")
            .expect("event should exist");
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

    #[test]
    fn test_accumulated_samples_add_sample_dispatches_variants() {
        let mut acc = AccumulatedSamples::new();

        acc.add_sample(Sample::Resource(SampleResource {
            attributes: vec![SampleAttribute {
                name: "service.name".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("checkout")),
                live_check_result: None,
            }],
            live_check_result: None,
        }));
        acc.add_sample(Sample::Attribute(SampleAttribute {
            name: "deployment.environment".to_owned(),
            r#type: Some(PrimitiveOrArrayTypeSpec::String),
            value: Some(json!("prod")),
            live_check_result: None,
        }));
        acc.add_sample(Sample::Span(SampleSpan {
            name: "ProcessOrder".to_owned(),
            kind: SpanKindSpec::Server,
            status: None,
            attributes: vec![],
            span_events: vec![],
            span_links: vec![],
            live_check_result: None,
            resource: None,
        }));
        acc.add_sample(Sample::Metric(SampleMetric {
            name: "requests.total".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Counter),
            unit: "{request}".to_owned(),
            data_points: None,
            live_check_result: None,
            resource: None,
        }));
        acc.add_sample(Sample::Log(SampleLog {
            event_name: "user.login".to_owned(),
            severity_number: Some(9),
            severity_text: Some("INFO".to_owned()),
            body: Some("login success".to_owned()),
            attributes: vec![],
            trace_id: None,
            span_id: None,
            live_check_result: None,
            resource: None,
        }));
        acc.add_sample(Sample::SpanEvent(SampleSpanEvent {
            name: "ignored".to_owned(),
            attributes: vec![],
            live_check_result: None,
        }));
        acc.add_sample(Sample::SpanLink(SampleSpanLink {
            attributes: vec![],
            live_check_result: None,
        }));

        assert_eq!(acc.stats(), (2, 1, 1, 1));
        assert!(acc.resources.contains_key("service.name"));
        assert!(acc.resources.contains_key("deployment.environment"));
        assert!(acc.spans.contains_key("ProcessOrder"));
        assert!(acc.metrics.contains_key("requests.total"));
        assert!(acc.events.contains_key("user.login"));
    }

    #[test]
    fn test_accumulated_samples_add_metric_ignores_unsupported_instrument() {
        let mut acc = AccumulatedSamples::new();

        acc.add_metric(SampleMetric {
            name: "unsupported.summary".to_owned(),
            instrument: SampleInstrument::Unsupported("Summary".to_owned()),
            unit: String::new(),
            data_points: None,
            live_check_result: None,
            resource: None,
        });

        assert!(acc.is_empty());
        assert!(!acc.metrics.contains_key("unsupported.summary"));
    }

    #[test]
    fn test_accumulated_samples_add_metric_collects_all_data_point_attributes() {
        let mut acc = AccumulatedSamples::new();

        acc.add_metric(SampleMetric {
            name: "http.server.active_requests".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::UpDownCounter),
            unit: "{request}".to_owned(),
            data_points: Some(DataPoints::Number(vec![SampleNumberDataPoint {
                attributes: vec![SampleAttribute {
                    name: "server.address".to_owned(),
                    r#type: Some(PrimitiveOrArrayTypeSpec::String),
                    value: Some(json!("api.example.com")),
                    live_check_result: None,
                }],
                value: json!(3),
                flags: 0,
                exemplars: vec![],
                live_check_result: None,
            }])),
            live_check_result: None,
            resource: None,
        });
        acc.add_metric(SampleMetric {
            name: "http.server.duration".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Histogram),
            unit: "ms".to_owned(),
            data_points: Some(DataPoints::Histogram(vec![SampleHistogramDataPoint {
                attributes: vec![SampleAttribute {
                    name: "http.request.method".to_owned(),
                    r#type: Some(PrimitiveOrArrayTypeSpec::String),
                    value: Some(json!("GET")),
                    live_check_result: None,
                }],
                count: 1,
                sum: Some(12.5),
                bucket_counts: vec![1],
                explicit_bounds: vec![50.0],
                min: Some(12.5),
                max: Some(12.5),
                flags: 0,
                exemplars: vec![],
                live_check_result: None,
            }])),
            live_check_result: None,
            resource: None,
        });
        acc.add_metric(SampleMetric {
            name: "queue.latency".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Histogram),
            unit: "s".to_owned(),
            data_points: Some(DataPoints::ExponentialHistogram(vec![
                SampleExponentialHistogramDataPoint {
                    attributes: vec![SampleAttribute {
                        name: "queue.name".to_owned(),
                        r#type: Some(PrimitiveOrArrayTypeSpec::String),
                        value: Some(json!("payments")),
                        live_check_result: None,
                    }],
                    count: 1,
                    sum: Some(0.75),
                    scale: 0,
                    zero_count: 0,
                    positive: None,
                    negative: None,
                    flags: 0,
                    min: Some(0.75),
                    max: Some(0.75),
                    zero_threshold: 0.0,
                    exemplars: vec![],
                    live_check_result: None,
                },
            ])),
            live_check_result: None,
            resource: None,
        });

        assert_eq!(acc.stats(), (0, 0, 3, 0));
        assert!(acc.metrics["http.server.active_requests"]
            .attributes
            .contains_key("server.address"));
        assert!(acc.metrics["http.server.duration"]
            .attributes
            .contains_key("http.request.method"));
        assert!(acc.metrics["queue.latency"]
            .attributes
            .contains_key("queue.name"));
    }

    // ============================================
    // Tests for to_semconv_spec()
    // ============================================

    #[test]
    fn test_to_semconv_spec_empty_accumulator() {
        let acc = AccumulatedSamples::new();
        let registry = acc.to_semconv_spec();

        assert_eq!(registry.file_format, "definition/2");
        assert!(registry.attributes.is_empty());
        assert!(registry.entities.is_empty());
        assert!(registry.spans.is_empty());
        assert!(registry.metrics.is_empty());
        assert!(registry.events.is_empty());
    }

    #[test]
    fn test_to_semconv_spec_with_resources_creates_entity_identity() {
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

        assert_eq!(registry.attributes.len(), 1);
        assert_eq!(registry.attributes[0].key, "service.name");
        assert_eq!(registry.entities.len(), 1);
        assert_eq!(registry.entities[0].r#type.to_string(), "resource");
        assert_eq!(registry.entities[0].identity.len(), 1);
        assert_eq!(registry.entities[0].identity[0].r#ref, "service.name");
        assert!(registry.entities[0].description.is_empty());
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
            resource: None,
        });

        let registry = acc.to_semconv_spec();

        assert_eq!(registry.attributes.len(), 1);
        assert_eq!(registry.attributes[0].key, "http.url");
        assert_eq!(registry.spans.len(), 1);
        assert_eq!(registry.spans[0].r#type.to_string(), "HTTP GET");
        assert_eq!(registry.spans[0].kind, SpanKindSpec::Client);
        assert_eq!(registry.spans[0].name.note, "HTTP GET");
        assert_eq!(registry.spans[0].attributes.len(), 1);
        match &registry.spans[0].attributes[0] {
            SpanAttributeOrGroupRef::Attribute(attribute) => {
                assert_eq!(attribute.base.r#ref, "http.url");
            }
            SpanAttributeOrGroupRef::Group(_) => panic!("expected attribute ref"),
        }
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
            resource: None,
        };

        acc.add_metric(metric);

        let registry = acc.to_semconv_spec();

        assert!(registry.attributes.is_empty());
        assert_eq!(registry.metrics.len(), 1);
        let metric = &registry.metrics[0];
        assert_eq!(metric.name.to_string(), "http.server.duration");
        assert_eq!(metric.instrument, InstrumentSpec::Histogram);
        assert_eq!(metric.unit, "ms");
    }

    #[test]
    fn test_to_semconv_spec_metric_empty_unit_is_empty_string() {
        let mut acc = AccumulatedSamples::new();

        let metric = SampleMetric {
            name: "custom.counter".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Counter),
            unit: String::new(), // Empty unit
            data_points: None,
            live_check_result: None,
            resource: None,
        };

        acc.add_metric(metric);

        let registry = acc.to_semconv_spec();

        assert_eq!(registry.metrics[0].unit, String::new());
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

        assert_eq!(registry.attributes.len(), 1);
        assert_eq!(registry.attributes[0].key, "user.email");
        assert_eq!(registry.events.len(), 1);
        assert_eq!(registry.events[0].name.to_string(), "user.signup");
    }

    #[test]
    fn test_to_semconv_spec_with_span_event_group() {
        let mut acc = AccumulatedSamples::new();

        acc.add_span(SampleSpan {
            name: "HandleCheckout".to_owned(),
            kind: SpanKindSpec::Server,
            status: None,
            attributes: vec![],
            span_events: vec![SampleSpanEvent {
                name: "exception".to_owned(),
                attributes: vec![
                    SampleAttribute {
                        name: "z.attr".to_owned(),
                        r#type: Some(PrimitiveOrArrayTypeSpec::String),
                        value: Some(json!("last")),
                        live_check_result: None,
                    },
                    SampleAttribute {
                        name: "a.attr".to_owned(),
                        r#type: Some(PrimitiveOrArrayTypeSpec::String),
                        value: Some(json!("first")),
                        live_check_result: None,
                    },
                ],
                live_check_result: None,
            }],
            span_links: vec![],
            live_check_result: None,
            resource: None,
        });

        let registry = acc.to_semconv_spec();

        assert_eq!(registry.spans.len(), 1);
        assert_eq!(registry.events.len(), 1);
        let span_event = &registry.events[0];
        assert_eq!(span_event.name.to_string(), "exception");
        let attr_ids: Vec<_> = span_event
            .attributes
            .iter()
            .map(attribute_or_group_ref_name)
            .collect();
        assert_eq!(attr_ids, vec!["a.attr", "z.attr"]);
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

        let attr_ids: Vec<_> = registry
            .attributes
            .iter()
            .map(|attribute| attribute.key.as_str())
            .collect();
        assert_eq!(attr_ids, vec!["a.attr", "m.attr", "z.attr"]);
    }

    #[test]
    fn test_to_semconv_spec_deduplicates_attributes_across_signals() {
        let mut acc = AccumulatedSamples::new();

        accumulate_attribute(
            &mut acc.resources,
            SampleAttribute {
                name: "shared.attr".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("resource")),
                live_check_result: None,
            },
        );
        acc.add_event(
            "shared.event".to_owned(),
            vec![SampleAttribute {
                name: "shared.attr".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("event")),
                live_check_result: None,
            }],
        );

        let registry = acc.to_semconv_spec();

        assert_eq!(registry.attributes.len(), 1);
        assert_eq!(registry.attributes[0].key, "shared.attr");
        assert_eq!(registry.entities[0].identity[0].r#ref, "shared.attr");
        assert_eq!(
            attribute_or_group_ref_name(&registry.events[0].attributes[0]),
            "shared.attr"
        );
    }

    #[test]
    fn test_to_semconv_spec_merges_log_and_span_events_with_same_name() {
        let mut acc = AccumulatedSamples::new();

        acc.add_event(
            "exception".to_owned(),
            vec![SampleAttribute {
                name: "log.attr".to_owned(),
                r#type: Some(PrimitiveOrArrayTypeSpec::String),
                value: Some(json!("log")),
                live_check_result: None,
            }],
        );
        acc.add_span(SampleSpan {
            name: "HandleCheckout".to_owned(),
            kind: SpanKindSpec::Server,
            status: None,
            attributes: vec![],
            span_events: vec![SampleSpanEvent {
                name: "exception".to_owned(),
                attributes: vec![SampleAttribute {
                    name: "span.attr".to_owned(),
                    r#type: Some(PrimitiveOrArrayTypeSpec::String),
                    value: Some(json!("span")),
                    live_check_result: None,
                }],
                live_check_result: None,
            }],
            span_links: vec![],
            live_check_result: None,
            resource: None,
        });

        let registry = acc.to_semconv_spec();

        assert_eq!(registry.events.len(), 1);
        let attr_ids: Vec<_> = registry.events[0]
            .attributes
            .iter()
            .map(attribute_or_group_ref_name)
            .collect();
        assert_eq!(attr_ids, vec!["log.attr", "span.attr"]);
    }
}
