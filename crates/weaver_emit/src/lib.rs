// SPDX-License-Identifier: Apache-2.0

//! This crate provides the "emit" library for emitting OTLP signals generated from registries.

use miette::Diagnostic;
use opentelemetry::{global, trace::TraceError, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{trace as sdktrace, Resource};
use serde::Serialize;
use spans::emit_trace_for_registry;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_forge::registry::ResolvedRegistry;

pub mod spans;

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
fn init_tracer_provider() -> Result<sdktrace::TracerProvider, TraceError> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint("http://localhost:4317")
        .build()?;
    Ok(sdktrace::TracerProvider::builder()
        .with_resource(Resource::new(vec![KeyValue::new("service.name", "weaver")]))
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .build())
}

/// Initialise a stdout exporter for debug
fn init_stdout_tracer_provider() -> sdktrace::TracerProvider {
    sdktrace::TracerProvider::builder()
        .with_resource(Resource::new(vec![KeyValue::new("service.name", "weaver")]))
        .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
        .build()
}

/// The configuration for the tracer provider.
pub enum ExporterConfig {
    /// Emit to stdout.
    Stdout,
    /// Emit to OTLP.
    Otlp,
}

/// Emit the signals from the registry to the configured exporter.
pub fn emit(
    registry: &ResolvedRegistry,
    registry_path: &str,
    tracer_provider_config: &ExporterConfig,
) -> Result<(), Error> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| Error::EmitError {
        error: e.to_string(),
    })?; //.expect("failed to create tokio runtime");
    rt.block_on(async {
        // Emit spans
        let tracer_provider = match tracer_provider_config {
            ExporterConfig::Stdout => init_stdout_tracer_provider(),
            ExporterConfig::Otlp => {
                init_tracer_provider().map_err(|e| Error::TracerProviderError {
                    error: e.to_string(),
                })? //.expect("OTLP Tracer Provider must be created")
            }
        };
        let _ = global::set_tracer_provider(tracer_provider.clone());

        emit_trace_for_registry(registry, registry_path);

        global::shutdown_tracer_provider();

        // TODO Emit metrics
        Ok(())
    })
}
