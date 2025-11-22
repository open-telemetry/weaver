// SPDX-License-Identifier: Apache-2.0

//! Translations from Weaver to Otel for spans.

use crate::attributes::{get_attribute_name_value, get_attribute_name_value_v2};
use opentelemetry::{
    global,
    trace::{SpanKind, TraceContextExt, Tracer},
    KeyValue,
};
use weaver_forge::{
    registry::ResolvedRegistry,
    v2::{registry::ForgeResolvedRegistry, span::SpanAttribute},
};
use weaver_semconv::group::{GroupType, SpanKindSpec};

// TODO These constants should be replaced with official semconvs when available.
const WEAVER_EMIT_SPAN: &str = "otel.weaver.emit";
const WEAVER_REGISTRY_PATH: &str = "otel.weaver.registry_path";

/// Convert the Weaver span kind to an OTLP span kind.
/// If the span kind is not specified, return `SpanKind::Internal`.
#[must_use]
fn otel_span_kind(span_kind: Option<&SpanKindSpec>) -> SpanKind {
    match span_kind {
        Some(SpanKindSpec::Client) => SpanKind::Client,
        Some(SpanKindSpec::Server) => SpanKind::Server,
        Some(SpanKindSpec::Producer) => SpanKind::Producer,
        Some(SpanKindSpec::Consumer) => SpanKind::Consumer,
        Some(SpanKindSpec::Internal) | None => SpanKind::Internal,
    }
}

/// Uses the global tracer_provider to emit a single trace for all the defined
/// spans in the registry
pub(crate) fn emit_trace_for_registry(registry: &ResolvedRegistry, registry_path: &str) {
    let tracer = global::tracer("weaver");
    // Start a parent span here and use this context to create child spans
    tracer.in_span(WEAVER_EMIT_SPAN, |cx| {
        let span = cx.span();
        span.set_attribute(KeyValue::new(
            WEAVER_REGISTRY_PATH,
            registry_path.to_owned(),
        ));

        // Emit each span to the OTLP receiver.
        for group in registry.groups.iter() {
            if group.r#type == GroupType::Span {
                let _span = tracer
                    .span_builder(group.id.clone())
                    .with_kind(otel_span_kind(group.span_kind.as_ref()))
                    .with_attributes(group.attributes.iter().map(get_attribute_name_value))
                    .start_with_context(&tracer, &cx);
            }
        }
    });
}

pub(crate) fn emit_trace_for_registry_v2(registry: &ForgeResolvedRegistry, registry_path: &str) {
    let tracer = global::tracer("weaver");
    // Start a parent span here and use this context to create child spans
    tracer.in_span(WEAVER_EMIT_SPAN, |cx| {
        let span = cx.span();
        span.set_attribute(KeyValue::new(
            WEAVER_REGISTRY_PATH,
            registry_path.to_owned(),
        ));

        // Emit each span to the OTLP receiver.
        for span in registry.signals.spans.iter() {
            let _span =
                tracer
                    .span_builder(span.r#type.to_string())
                    .with_kind(otel_span_kind(Some(&span.kind)))
                    .with_attributes(span.attributes.iter().map(|span_attr: &SpanAttribute| {
                        get_attribute_name_value_v2(&span_attr.base)
                    }))
                    .start_with_context(&tracer, &cx);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_kinds() {
        assert_eq!(
            otel_span_kind(Some(&SpanKindSpec::Client)),
            SpanKind::Client
        );
        assert_eq!(
            otel_span_kind(Some(&SpanKindSpec::Server)),
            SpanKind::Server
        );
        assert_eq!(
            otel_span_kind(Some(&SpanKindSpec::Producer)),
            SpanKind::Producer
        );
        assert_eq!(
            otel_span_kind(Some(&SpanKindSpec::Consumer)),
            SpanKind::Consumer
        );
        assert_eq!(
            otel_span_kind(Some(&SpanKindSpec::Internal)),
            SpanKind::Internal
        );
        assert_eq!(otel_span_kind(None), SpanKind::Internal);
    }
}
