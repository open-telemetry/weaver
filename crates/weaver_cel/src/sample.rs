// SPDX-License-Identifier: Apache-2.0

//! A minimal telemetry sample model, standing in for the live-check samples.
//!
//! This is deliberately self-contained so the CEL experiments do not need the
//! rest of Weaver. It carries only what a matcher expression can read, and
//! only the sample types the plan gives an example for.

use std::collections::HashMap;

use cel::{Context, Value};
use serde::Deserialize;

/// A telemetry sample, tagged by its sample type.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sample {
    /// A span sample.
    Span(Span),
    /// A log record sample.
    Log(Log),
    /// A metric sample, with its data points.
    Metric(Metric),
    /// A resource sample.
    Resource(Resource),
    /// An instrumentation scope sample.
    InstrumentationScope(InstrumentationScope),
}

impl Sample {
    /// The `sample_type` a matcher has to declare to see this sample.
    #[must_use]
    pub fn sample_type(&self) -> SampleType {
        match self {
            Sample::Span(_) => SampleType::Span,
            Sample::Log(_) => SampleType::Log,
            Sample::Metric(_) => SampleType::Metric,
            Sample::Resource(_) => SampleType::Resource,
            Sample::InstrumentationScope(_) => SampleType::InstrumentationScope,
        }
    }

    /// Binds the variables a matcher expression can read for this sample.
    pub fn bind(&self, context: &mut Context<'_>) {
        match self {
            Sample::Span(span) => span.bind(context),
            Sample::Log(log) => log.bind(context),
            Sample::Metric(metric) => metric.bind(context),
            Sample::Resource(resource) => resource.bind(context),
            Sample::InstrumentationScope(scope) => scope.bind(context),
        }
    }
}

/// The kind of sample a matcher looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleType {
    /// A span.
    Span,
    /// A log record.
    Log,
    /// A metric.
    Metric,
    /// A resource.
    Resource,
    /// An instrumentation scope.
    InstrumentationScope,
}

/// The context a signal sample arrived in.
///
/// Absent parts are left unbound, so an expression that reads one errors
/// rather than quietly coming out false.
#[derive(Debug, Default, Deserialize)]
pub struct SignalContext {
    /// The resource the sample arrived with.
    pub resource: Option<Resource>,
    /// The scope that produced the sample.
    pub instrumentation_scope: Option<InstrumentationScope>,
}

impl SignalContext {
    fn bind(&self, context: &mut Context<'_>) {
        if let Some(resource) = &self.resource {
            context.add_variable_from_value("resource", resource.to_value());
        }
        if let Some(scope) = &self.instrumentation_scope {
            context.add_variable_from_value("instrumentation_scope", scope.to_value());
        }
    }
}

/// A span sample.
#[derive(Debug, Deserialize)]
pub struct Span {
    /// The span name. Free-form, which is why spans need a matcher at all.
    pub name: String,
    /// The span kind, e.g. `internal`.
    pub kind: String,
    /// The outcome of the span. A span with no status is `unset`, as in OTLP.
    #[serde(default)]
    pub status: Status,
    /// The attributes on the span.
    pub attributes: Vec<Attribute>,
    /// The resource and scope the span arrived with.
    #[serde(flatten)]
    pub signal_context: SignalContext,
}

impl Span {
    fn bind(&self, context: &mut Context<'_>) {
        context.add_variable_from_value("name", self.name.as_str());
        context.add_variable_from_value("kind", self.kind.as_str());
        context.add_variable_from_value("status", self.status.to_value());
        context.add_variable_from_value("attributes", attribute_map(&self.attributes));
        self.signal_context.bind(context);
    }
}

/// The status of a span, as OTLP carries it.
#[derive(Debug, Default, Deserialize)]
pub struct Status {
    /// The status code.
    #[serde(default)]
    pub code: StatusCode,
    /// The status message.
    #[serde(default)]
    pub message: String,
}

impl Status {
    fn to_value(&self) -> HashMap<String, Value> {
        HashMap::from([
            ("code".to_owned(), Value::from(self.code.as_str())),
            ("message".to_owned(), Value::from(self.message.as_str())),
        ])
    }
}

/// The status code of a span.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusCode {
    /// No status was set, which is the OTLP default.
    #[default]
    Unset,
    /// The operation succeeded.
    Ok,
    /// The operation failed.
    Error,
}

impl StatusCode {
    /// The name a matcher expression compares with.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            StatusCode::Unset => "unset",
            StatusCode::Ok => "ok",
            StatusCode::Error => "error",
        }
    }
}

/// A log record sample.
#[derive(Debug, Deserialize)]
pub struct Log {
    /// The event name. Empty when the log does not name an event, as in OTLP.
    #[serde(default)]
    pub event_name: String,
    /// The severity text.
    #[serde(default)]
    pub severity_text: String,
    /// The severity number.
    #[serde(default)]
    pub severity_number: i64,
    /// The log body, as a string.
    #[serde(default)]
    pub body: String,
    /// The attributes on the log record.
    pub attributes: Vec<Attribute>,
    /// The resource and scope the log arrived with.
    #[serde(flatten)]
    pub signal_context: SignalContext,
}

impl Log {
    fn bind(&self, context: &mut Context<'_>) {
        context.add_variable_from_value("event_name", self.event_name.as_str());
        context.add_variable_from_value("severity_text", self.severity_text.as_str());
        context.add_variable_from_value("severity_number", self.severity_number);
        context.add_variable_from_value("body", self.body.as_str());
        context.add_variable_from_value("attributes", attribute_map(&self.attributes));
        self.signal_context.bind(context);
    }
}

/// A metric sample.
#[derive(Debug, Deserialize)]
pub struct Metric {
    /// The metric name.
    pub name: String,
    /// The unit of the metric.
    pub unit: String,
    /// The instrument of the metric, e.g. `histogram`.
    pub instrument: String,
    /// The data points on the metric.
    #[serde(default)]
    pub data_points: Vec<DataPoint>,
    /// The resource and scope the metric arrived with.
    #[serde(flatten)]
    pub signal_context: SignalContext,
}

impl Metric {
    fn bind(&self, context: &mut Context<'_>) {
        context.add_variable_from_value("name", self.name.as_str());
        context.add_variable_from_value("unit", self.unit.as_str());
        context.add_variable_from_value("instrument", self.instrument.as_str());
        // On a metric, `attributes` is all of the data point attributes
        // together.
        let attributes: HashMap<String, Value> = self
            .data_points
            .iter()
            .flat_map(|data_point| data_point.attributes.iter())
            .map(entry)
            .collect();
        context.add_variable_from_value("attributes", attributes);
        self.signal_context.bind(context);
    }
}

/// One data point on a metric.
#[derive(Debug, Deserialize)]
pub struct DataPoint {
    /// The attributes on the data point.
    #[serde(default)]
    pub attributes: Vec<Attribute>,
}

/// A resource, either as a sample of its own or as the context of a signal.
#[derive(Debug, Deserialize)]
pub struct Resource {
    /// The attributes on the resource.
    pub attributes: Vec<Attribute>,
}

impl Resource {
    fn bind(&self, context: &mut Context<'_>) {
        context.add_variable_from_value("attributes", attribute_map(&self.attributes));
    }

    fn to_value(&self) -> HashMap<String, Value> {
        HashMap::from([(
            "attributes".to_owned(),
            Value::from(attribute_map(&self.attributes)),
        )])
    }
}

/// An instrumentation scope, either as a sample of its own or as the context
/// of a signal.
#[derive(Debug, Deserialize)]
pub struct InstrumentationScope {
    /// The scope name, e.g. `io.opentelemetry.jdbc`.
    pub name: String,
    /// The scope version.
    #[serde(default)]
    pub version: String,
    /// The schema the telemetry claims to follow.
    #[serde(default)]
    pub schema_url: String,
    /// The attributes on the scope.
    #[serde(default)]
    pub attributes: Vec<Attribute>,
}

impl InstrumentationScope {
    fn bind(&self, context: &mut Context<'_>) {
        context.add_variable_from_value("name", self.name.as_str());
        context.add_variable_from_value("version", self.version.as_str());
        context.add_variable_from_value("schema_url", self.schema_url.as_str());
        context.add_variable_from_value("attributes", attribute_map(&self.attributes));
    }

    fn to_value(&self) -> HashMap<String, Value> {
        HashMap::from([
            ("name".to_owned(), Value::from(self.name.as_str())),
            ("version".to_owned(), Value::from(self.version.as_str())),
            (
                "schema_url".to_owned(),
                Value::from(self.schema_url.as_str()),
            ),
            (
                "attributes".to_owned(),
                Value::from(attribute_map(&self.attributes)),
            ),
        ])
    }
}

/// A single attribute on a sample.
#[derive(Debug, Deserialize)]
pub struct Attribute {
    /// The attribute key.
    pub name: String,
    /// The attribute value.
    pub value: AttributeValue,
}

/// The value of an attribute, in the few shapes JSON gives us.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    /// A boolean value.
    Bool(bool),
    /// An integer value.
    Int(i64),
    /// A floating point value.
    Double(f64),
    /// A string value.
    String(String),
}

impl From<&AttributeValue> for Value {
    fn from(value: &AttributeValue) -> Self {
        match value {
            AttributeValue::Bool(v) => Value::Bool(*v),
            AttributeValue::Int(v) => Value::Int(*v),
            AttributeValue::Double(v) => Value::Float(*v),
            AttributeValue::String(v) => Value::String(v.clone().into()),
        }
    }
}

/// Builds the `attributes` map a matcher expression indexes into.
fn attribute_map(attributes: &[Attribute]) -> HashMap<String, Value> {
    attributes.iter().map(entry).collect()
}

fn entry(attribute: &Attribute) -> (String, Value) {
    (attribute.name.clone(), Value::from(&attribute.value))
}
