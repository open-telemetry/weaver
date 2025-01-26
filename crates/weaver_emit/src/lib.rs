// SPDX-License-Identifier: Apache-2.0

//! This crate provides the "emit" library for emitting OTLP signals generated from registries.

use opentelemetry::{global, trace::TraceError, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{trace as sdktrace, Resource};
use spans::emit_trace_for_registry;
use weaver_forge::registry::ResolvedRegistry;

pub mod spans;

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
) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        // Emit spans
        let tracer_provider = match tracer_provider_config {
            ExporterConfig::Stdout => init_stdout_tracer_provider(),
            ExporterConfig::Otlp => {
                init_tracer_provider().expect("OTLP Tracer Provider must be created")
            }
        };
        let _ = global::set_tracer_provider(tracer_provider.clone());

        emit_trace_for_registry(registry, registry_path);

        global::shutdown_tracer_provider();

        // TODO Emit metrics
    });
}
