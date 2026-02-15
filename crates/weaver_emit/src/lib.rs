// SPDX-License-Identifier: Apache-2.0

//! This crate provides the "emit" library for emitting OTLP signals generated from registries.

use metrics::emit_metrics_for_registry;
use miette::Diagnostic;
use opentelemetry::global;
use opentelemetry_otlp::{ExporterBuildError, MetricExporter, WithExportConfig};
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::{metrics::PeriodicReader, trace::SdkTracerProvider};
use serde::Serialize;
use spans::emit_trace_for_registry;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_forge::registry::ResolvedRegistry;
use weaver_forge::v2::registry::ForgeResolvedRegistry;

use crate::logs::{emit_logs_for_registry, emit_logs_for_registry_v2};
use crate::metrics::emit_metrics_for_registry_v2;
use crate::spans::emit_trace_for_registry_v2;

pub mod attributes;
pub mod logs;
pub mod metrics;
pub mod spans;

/// The default OTLP endpoint.
pub const DEFAULT_OTLP_ENDPOINT: &str = "http://localhost:4317";

const WEAVER_SERVICE_NAME: &str = "weaver";

/// An error that can occur while emitting a semantic convention registry.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// Generic emit error.
    #[error("Fatal error during emit. {error}")]
    EmitError {
        /// The error that occurred.
        error: String,
    },
    /// Tracer provider error.
    #[error("Tracer provider error. Check your Otel configuration. {error}")]
    TracerProviderError {
        /// The error that occurred.
        error: String,
    },
    /// Metric provider error.
    #[error("Metric provider error. Check your Otel configuration. {error}")]
    MetricProviderError {
        /// The error that occurred.
        error: String,
    },
    /// Log provider error.
    #[error("Log provider error. Check your Otel configuration. {error}")]
    LogProviderError {
        /// The error that occurred.
        error: String,
    },
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// Initialise a grpc OTLP exporter, sends to by default http://localhost:4317
/// but can be overridden with the standard OTEL_EXPORTER_OTLP_ENDPOINT env var.
fn init_tracer_provider(endpoint: &String) -> Result<SdkTracerProvider, ExporterBuildError> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()?;
    Ok(SdkTracerProvider::builder()
        .with_resource(
            Resource::builder()
                .with_service_name(WEAVER_SERVICE_NAME)
                .build(),
        )
        .with_batch_exporter(exporter)
        .build())
}

/// Initialise a stdout exporter for debug
fn init_stdout_tracer_provider() -> SdkTracerProvider {
    SdkTracerProvider::builder()
        .with_resource(
            Resource::builder()
                .with_service_name(WEAVER_SERVICE_NAME)
                .build(),
        )
        .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
        .build()
}

/// Initialise a grpc OTLP exporter for metrics, sends to by default http://localhost:4317
/// but can be overridden with the standard OTEL_EXPORTER_OTLP_ENDPOINT env var.
fn init_meter_provider(endpoint: &String) -> Result<SdkMeterProvider, ExporterBuildError> {
    let resource = Resource::builder()
        .with_service_name(WEAVER_SERVICE_NAME)
        .build();

    let exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()?;

    let reader = PeriodicReader::builder(exporter).build();

    Ok(SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(reader)
        .build())
}

/// Initialise a stdout exporter for debug
fn init_stdout_meter_provider() -> SdkMeterProvider {
    let resource = Resource::builder()
        .with_service_name(WEAVER_SERVICE_NAME)
        .build();

    let exporter = opentelemetry_stdout::MetricExporter::default();
    let reader = PeriodicReader::builder(exporter).build();

    SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(reader)
        .build()
}

/// Initialise a grpc OTLP exporter for logs, sends to by default http://localhost:4317
/// but can be overridden with the standard OTEL_EXPORTER_OTLP_ENDPOINT env var.
fn init_logger_provider(
    endpoint: &String,
) -> Result<opentelemetry_sdk::logs::SdkLoggerProvider, ExporterBuildError> {
    let resource = Resource::builder()
        .with_service_name(WEAVER_SERVICE_NAME)
        .build();

    let exporter = opentelemetry_otlp::LogExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()?;

    Ok(opentelemetry_sdk::logs::SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build())
}

/// Initialise a stdout exporter for debug
fn init_stdout_logger_provider() -> opentelemetry_sdk::logs::SdkLoggerProvider {
    let resource = Resource::builder()
        .with_service_name(WEAVER_SERVICE_NAME)
        .build();

    opentelemetry_sdk::logs::SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_simple_exporter(opentelemetry_stdout::LogExporter::default())
        .build()
}

/// The configuration for the tracer provider.
#[derive(Debug)]
pub enum ExporterConfig {
    /// Emit to stdout.
    Stdout,
    /// Emit to OTLP.
    Otlp {
        /// The endpoint to emit to.
        endpoint: String,
    },
}

/// Enum for the registry: ResolvedRegistry or ForgeResolvedRegistry
#[derive(Debug)]
pub enum RegistryVersion<'a> {
    /// v1 ResolvedRegistry
    V1(&'a ResolvedRegistry),
    /// v2 ForgeResolvedRegistry
    V2(&'a ForgeResolvedRegistry),
}

/// Emit the signals from the registry to the configured exporter.
pub fn emit(
    registry: RegistryVersion<'_>,
    registry_path: &str,
    exporter_config: &ExporterConfig,
) -> Result<(), Error> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| Error::EmitError {
        error: e.to_string(),
    })?;
    rt.block_on(async {
        // Emit spans
        let tracer_provider = match exporter_config {
            ExporterConfig::Stdout => init_stdout_tracer_provider(),
            ExporterConfig::Otlp { endpoint } => {
                init_tracer_provider(endpoint).map_err(|e| Error::TracerProviderError {
                    error: e.to_string(),
                })?
            }
        };
        global::set_tracer_provider(tracer_provider.clone());

        match registry {
            RegistryVersion::V1(reg) => emit_trace_for_registry(reg, registry_path),
            RegistryVersion::V2(reg) => emit_trace_for_registry_v2(reg, registry_path),
        }

        tracer_provider
            .force_flush()
            .map_err(|e| Error::TracerProviderError {
                error: e.to_string(),
            })?;

        // Emit metrics
        let meter_provider = match exporter_config {
            ExporterConfig::Stdout => init_stdout_meter_provider(),
            ExporterConfig::Otlp { endpoint } => {
                init_meter_provider(endpoint).map_err(|e| Error::MetricProviderError {
                    error: e.to_string(),
                })?
            }
        };
        global::set_meter_provider(meter_provider.clone());

        match registry {
            RegistryVersion::V1(reg) => emit_metrics_for_registry(reg),
            RegistryVersion::V2(reg) => emit_metrics_for_registry_v2(reg),
        }

        meter_provider
            .shutdown()
            .map_err(|e| Error::MetricProviderError {
                error: e.to_string(),
            })?;

        // Emit logs
        let logger_provider = match exporter_config {
            ExporterConfig::Stdout => init_stdout_logger_provider(),
            ExporterConfig::Otlp { endpoint } => {
                init_logger_provider(endpoint).map_err(|e| Error::LogProviderError {
                    error: e.to_string(),
                })?
            }
        };

        match registry {
            RegistryVersion::V1(reg) => emit_logs_for_registry(reg, &logger_provider),
            RegistryVersion::V2(reg) => emit_logs_for_registry_v2(reg, &logger_provider),
        }

        logger_provider
            .shutdown()
            .map_err(|e| Error::LogProviderError {
                error: e.to_string(),
            })?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_forge::registry::{ResolvedGroup, ResolvedRegistry};
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::{
        attribute::{AttributeType, Examples, PrimitiveOrArrayTypeSpec, RequirementLevel},
        group::{GroupType, InstrumentSpec, SpanKindSpec},
        schema_url::SchemaUrl,
        stability::Stability,
    };

    // Test the emit command for stdout
    #[test]
    fn test_emit_stdout() {
        let registry = ResolvedRegistry {
            registry_url: "TEST".to_owned(),
            groups: vec![
                ResolvedGroup {
                    id: "test.comprehensive.internal".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![Attribute {
                        name: "test.string".to_owned(),
                        r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                        examples: Some(Examples::Strings(vec![
                            "value1".to_owned(),
                            "value2".to_owned(),
                        ])),
                        brief: "".to_owned(),
                        tag: None,
                        requirement_level: RequirementLevel::Recommended {
                            text: "".to_owned(),
                        },
                        sampling_relevant: None,
                        note: "".to_owned(),
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        prefix: false,
                        tags: None,
                        value: None,
                        annotations: None,
                        role: Default::default(),
                    }],
                    span_kind: Some(SpanKindSpec::Internal),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    entity_associations: vec![],
                    annotations: None,
                },
                ResolvedGroup {
                    id: "test.updowncounter".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "test.updowncounter".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    entity_associations: vec![],
                    extends: None,
                    stability: Some(Stability::Development),
                    deprecated: None,
                    attributes: vec![],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("test.updowncounter".to_owned()),
                    instrument: Some(InstrumentSpec::UpDownCounter),
                    unit: Some("1".to_owned()),
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                },
                ResolvedGroup {
                    id: "test.counter".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "test.counter".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    entity_associations: vec![],
                    extends: None,
                    stability: Some(Stability::Development),
                    deprecated: None,
                    attributes: vec![],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("test.counter".to_owned()),
                    instrument: Some(InstrumentSpec::Counter),
                    unit: Some("1".to_owned()),
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                },
                ResolvedGroup {
                    id: "test.gauge".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "test.gauge".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    entity_associations: vec![],
                    extends: None,
                    stability: Some(Stability::Development),
                    deprecated: None,
                    attributes: vec![],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("test.gauge".to_owned()),
                    instrument: Some(InstrumentSpec::Gauge),
                    unit: Some("1".to_owned()),
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                },
                ResolvedGroup {
                    id: "test.histogram".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "test.histogram".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    entity_associations: vec![],
                    extends: None,
                    stability: Some(Stability::Development),
                    deprecated: None,
                    attributes: vec![],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("test.histogram".to_owned()),
                    instrument: Some(InstrumentSpec::Histogram),
                    unit: Some("1".to_owned()),
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                },
                ResolvedGroup {
                    id: "test.updowncounter.double".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "test.updowncounter.double".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    entity_associations: vec![],
                    extends: None,
                    stability: Some(Stability::Development),
                    deprecated: None,
                    attributes: vec![],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("test.updowncounter.double".to_owned()),
                    instrument: Some(InstrumentSpec::UpDownCounter),
                    unit: Some("1".to_owned()),
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                },
                ResolvedGroup {
                    id: "test.counter.double".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "test.counter.double".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    entity_associations: vec![],
                    extends: None,
                    stability: Some(Stability::Development),
                    deprecated: None,
                    attributes: vec![],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("test.counter.double".to_owned()),
                    instrument: Some(InstrumentSpec::Counter),
                    unit: Some("1".to_owned()),
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                },
                ResolvedGroup {
                    id: "test.gauge.double".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "test.gauge.double".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    entity_associations: vec![],
                    extends: None,
                    stability: Some(Stability::Development),
                    deprecated: None,
                    attributes: vec![],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("test.gauge.double".to_owned()),
                    instrument: Some(InstrumentSpec::Gauge),
                    unit: Some("1".to_owned()),
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                },
                ResolvedGroup {
                    id: "test.histogram.double".to_owned(),
                    r#type: GroupType::Metric,
                    brief: "test.histogram.double".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    entity_associations: vec![],
                    extends: None,
                    stability: Some(Stability::Development),
                    deprecated: None,
                    attributes: vec![],
                    span_kind: None,
                    events: vec![],
                    metric_name: Some("test.histogram.double".to_owned()),
                    instrument: Some(InstrumentSpec::Histogram),
                    unit: Some("1".to_owned()),
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                },
                ResolvedGroup {
                    id: "event.session.start".to_owned(),
                    r#type: GroupType::Event,
                    brief: "This event represents a session start".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    entity_associations: vec![],
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![Attribute {
                        name: "session.id".to_owned(),
                        r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                        examples: Some(Examples::Strings(vec![
                            "00112233-4455-6677-8899-aabbccddeeff".to_owned(),
                        ])),
                        brief: "A unique session identifier".to_owned(),
                        tag: None,
                        requirement_level: RequirementLevel::Recommended {
                            text: "".to_owned(),
                        },
                        sampling_relevant: None,
                        note: "".to_owned(),
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        prefix: false,
                        tags: None,
                        value: None,
                        annotations: None,
                        role: Default::default(),
                    }],
                    span_kind: None,
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    name: Some("session.start".to_owned()),
                    lineage: None,
                    display_name: Some("Session Start Event".to_owned()),
                    body: None,
                    annotations: None,
                },
            ],
        };

        let result = emit(
            RegistryVersion::V1(&registry),
            "TEST",
            &ExporterConfig::Stdout,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_emit_otlp_invalid_endpoint() {
        let registry = ResolvedRegistry {
            registry_url: "TEST_OTLP_INVALID".to_owned(),
            groups: vec![],
        };
        let result = emit(
            RegistryVersion::V1(&registry),
            "TEST_OTLP_INVALID",
            &ExporterConfig::Otlp {
                endpoint: "http:/invalid-endpoint:4317".to_owned(),
            },
        );
        assert!(result.is_err());

        // Check the error converts to a diagnostic message
        let diagnostic_messages = DiagnosticMessages::from(result.unwrap_err());
        assert_eq!(diagnostic_messages.len(), 1);
    }

    #[test]
    fn test_emit_stdout_v2() {
        use std::collections::BTreeMap;
        use weaver_forge::v2::{
            attribute::Attribute as V2Attribute,
            event::{Event, EventAttribute},
            metric::Metric,
            registry::{ForgeResolvedRegistry, Refinements, Registry},
            span::{Span, SpanAttribute},
        };
        use weaver_semconv::{
            attribute::{
                AttributeType, BasicRequirementLevelSpec, Examples, PrimitiveOrArrayTypeSpec,
                RequirementLevel,
            },
            group::{InstrumentSpec, SpanKindSpec},
            stability::Stability,
            v2::{signal_id::SignalId, span::SpanName, CommonFields},
        };

        let registry = ForgeResolvedRegistry {
            schema_url: SchemaUrl::try_new("https://example.com/schemas/1.2.3".to_owned()).unwrap(),
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                spans: vec![Span {
                    r#type: SignalId::from("test.comprehensive.internal".to_owned()),
                    kind: SpanKindSpec::Internal,
                    name: SpanName {
                        note: "test span".to_owned(),
                    },
                    attributes: vec![SpanAttribute {
                        base: V2Attribute {
                            key: "test.string".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: Some(Examples::Strings(vec![
                                "value1".to_owned(),
                                "value2".to_owned(),
                            ])),
                            common: CommonFields {
                                brief: "Test attribute".to_owned(),
                                note: String::new(),
                                stability: Stability::Stable,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                        },
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Recommended,
                        ),
                        sampling_relevant: None,
                    }],
                    entity_associations: vec![],
                    common: CommonFields {
                        brief: "Test span".to_owned(),
                        note: String::new(),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                }],
                metrics: vec![
                    Metric {
                        name: SignalId::from("test.updowncounter".to_owned()),
                        instrument: InstrumentSpec::UpDownCounter,
                        unit: "1".to_owned(),
                        attributes: vec![],
                        entity_associations: vec![],
                        common: CommonFields {
                            brief: "test.updowncounter".to_owned(),
                            note: String::new(),
                            stability: Stability::Development,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                    },
                    Metric {
                        name: SignalId::from("test.counter".to_owned()),
                        instrument: InstrumentSpec::Counter,
                        unit: "1".to_owned(),
                        attributes: vec![],
                        entity_associations: vec![],
                        common: CommonFields {
                            brief: "test.counter".to_owned(),
                            note: String::new(),
                            stability: Stability::Development,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                    },
                    Metric {
                        name: SignalId::from("test.gauge".to_owned()),
                        instrument: InstrumentSpec::Gauge,
                        unit: "1".to_owned(),
                        attributes: vec![],
                        entity_associations: vec![],
                        common: CommonFields {
                            brief: "test.gauge".to_owned(),
                            note: String::new(),
                            stability: Stability::Development,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                    },
                    Metric {
                        name: SignalId::from("test.histogram".to_owned()),
                        instrument: InstrumentSpec::Histogram,
                        unit: "1".to_owned(),
                        attributes: vec![],
                        entity_associations: vec![],
                        common: CommonFields {
                            brief: "test.histogram".to_owned(),
                            note: String::new(),
                            stability: Stability::Development,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                    },
                ],
                events: vec![Event {
                    name: SignalId::from("session.start".to_owned()),
                    attributes: vec![EventAttribute {
                        base: V2Attribute {
                            key: "session.id".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: Some(Examples::Strings(vec![
                                "00112233-4455-6677-8899-aabbccddeeff".to_owned(),
                            ])),
                            common: CommonFields {
                                brief: "A unique session identifier".to_owned(),
                                note: String::new(),
                                stability: Stability::Stable,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                        },
                        requirement_level: RequirementLevel::Basic(
                            BasicRequirementLevelSpec::Recommended,
                        ),
                    }],
                    entity_associations: vec![],
                    common: CommonFields {
                        brief: "This event represents a session start".to_owned(),
                        note: String::new(),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                }],
                entities: vec![],
            },
            refinements: Refinements {
                metrics: vec![],
                spans: vec![],
                events: vec![],
            },
        };

        let result = emit(
            RegistryVersion::V2(&registry),
            "TEST_V2",
            &ExporterConfig::Stdout,
        );
        assert!(result.is_ok());
    }
}
