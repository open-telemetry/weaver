// SPDX-License-Identifier: Apache-2.0

//! This crate provides the "emit" library for emitting OTLP signals generated from registries.

use miette::Diagnostic;
use opentelemetry::{global, trace::TraceError};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use serde::Serialize;
use spans::emit_trace_for_registry;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_forge::registry::ResolvedRegistry;

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
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// Initialise a grpc OTLP exporter, sends to by default http://localhost:4317
/// but can be overridden with the standard OTEL_EXPORTER_OTLP_ENDPOINT env var.
fn init_tracer_provider(endpoint: &String) -> Result<SdkTracerProvider, TraceError> {
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

/// Emit the signals from the registry to the configured exporter.
pub fn emit(
    registry: &ResolvedRegistry,
    registry_path: &str,
    tracer_provider_config: &ExporterConfig,
) -> Result<(), Error> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| Error::EmitError {
        error: e.to_string(),
    })?;
    rt.block_on(async {
        // Emit spans
        let tracer_provider = match tracer_provider_config {
            ExporterConfig::Stdout => init_stdout_tracer_provider(),
            ExporterConfig::Otlp { endpoint } => {
                init_tracer_provider(endpoint).map_err(|e| Error::TracerProviderError {
                    error: e.to_string(),
                })?
            }
        };
        let _ = global::set_tracer_provider(tracer_provider.clone());

        emit_trace_for_registry(registry, registry_path);

        tracer_provider
            .force_flush()
            .map_err(|e| Error::TracerProviderError {
                error: e.to_string(),
            })?;

        // TODO Emit metrics
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
        group::{GroupType, SpanKindSpec},
        stability::Stability,
    };

    // Test the emit command for stdout
    #[test]
    fn test_emit_stdout() {
        let registry = ResolvedRegistry {
            registry_url: "TEST".to_owned(),
            groups: vec![ResolvedGroup {
                id: "test.comprehensive.internal".to_owned(),
                r#type: GroupType::Span,
                brief: "".to_owned(),
                note: "".to_owned(),
                prefix: "".to_owned(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                constraints: vec![],
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
            }],
        };

        let result = emit(&registry, "TEST", &ExporterConfig::Stdout);
        assert!(result.is_ok());
    }

    #[test]
    fn test_emit_otlp_invalid_endpoint() {
        let registry = ResolvedRegistry {
            registry_url: "TEST_OTLP_INVALID".to_owned(),
            groups: vec![],
        };
        let result = emit(
            &registry,
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
}
