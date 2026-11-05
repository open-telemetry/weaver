// SPDX-License-Identifier: Apache-2.0

//! Benchmarks for serializing live-check samples with shared context.

use std::{hint::black_box, rc::Rc, time::Duration};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde::Serialize;
use serde_json::Value;
use weaver_live_check::{
    sample_attribute::SampleAttribute, sample_instrumentation_scope::SampleInstrumentationScope,
    sample_resource::SampleResource, sample_span::SampleSpan, SampleRef,
};
use weaver_semconv::v1::group::SpanKindSpec;

#[derive(Serialize)]
struct BenchmarkRegoInput<'a> {
    sample: SampleRef<'a>,
    resource: Option<&'a SampleResource>,
    instrumentation_scope: Option<&'a SampleInstrumentationScope>,
    registry_attribute: Option<()>,
    registry_group: Option<()>,
}

struct Fixture {
    name: &'static str,
    span: SampleSpan,
}

impl Fixture {
    fn new(
        name: &'static str,
        resource_attribute_count: Option<usize>,
        scope_attribute_count: Option<usize>,
    ) -> Self {
        let resource = resource_attribute_count.map(|count| {
            Rc::new(SampleResource {
                attributes: attributes("resource", count),
                live_check_result: None,
            })
        });
        let instrumentation_scope = scope_attribute_count.map(|count| {
            Rc::new(SampleInstrumentationScope {
                name: "benchmark.scope".to_owned(),
                version: Some("1.0.0".to_owned()),
                schema_url: Some("https://opentelemetry.io/schemas/1.0.0".to_owned()),
                attributes: attributes("scope", count),
                dropped_attributes_count: 0,
                live_check_result: None,
            })
        });

        Self {
            name,
            span: SampleSpan {
                name: "benchmark.match".to_owned(),
                kind: SpanKindSpec::Internal,
                status: None,
                attributes: Vec::new(),
                span_events: Vec::new(),
                span_links: Vec::new(),
                instrumentation_scope,
                live_check_result: None,
                resource,
                trace_id: None,
                span_id: None,
                parent_span_id: None,
                trace_state: None,
                start_time: None,
                end_time: None,
            },
        }
    }

    fn input(&self) -> BenchmarkRegoInput<'_> {
        input_for_span(&self.span)
    }
}

fn attributes(context: &str, count: usize) -> Vec<SampleAttribute> {
    let mut attributes = Vec::with_capacity(count);
    for index in 0..count {
        attributes.push(SampleAttribute {
            name: format!("benchmark.{context}.attribute.{index:03}"),
            value: Some(Value::String(format!(
                "benchmark-{context}-value-{index:03}"
            ))),
            r#type: None,
            live_check_result: None,
        });
    }
    attributes
}

fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture::new("no_context/attrs_0", None, None),
        Fixture::new("resource_only/attrs_0", Some(0), None),
        Fixture::new("resource_only/attrs_16", Some(16), None),
        Fixture::new("resource_only/attrs_128", Some(128), None),
        Fixture::new("resource_and_scope/attrs_0", Some(0), Some(0)),
        Fixture::new("resource_and_scope/attrs_16", Some(16), Some(16)),
        Fixture::new("resource_and_scope/attrs_128", Some(128), Some(128)),
    ]
}

fn input_for_span(span: &SampleSpan) -> BenchmarkRegoInput<'_> {
    BenchmarkRegoInput {
        sample: SampleRef::Span(span),
        resource: span.resource.as_deref(),
        instrumentation_scope: span.instrumentation_scope.as_deref(),
        registry_attribute: None,
        registry_group: None,
    }
}

fn benchmark_sample_serialization(c: &mut Criterion) {
    let fixtures = fixtures();

    for fixture in &fixtures {
        let mut group = c.benchmark_group(format!("sample_serialization/{}", fixture.name));

        for batch_size in [1u64, 32, 256] {
            let _ = group.throughput(Throughput::Elements(batch_size));
            let _ = group.bench_function(
                BenchmarkId::new("serialize_input", format!("batch_{batch_size}")),
                |b| {
                    b.iter(|| {
                        for _ in 0..batch_size {
                            let input = fixture.input();
                            let serialized = serde_json::to_value(black_box(&input))
                                .expect("benchmark input must serialize");
                            let _ = black_box(serialized);
                        }
                    });
                },
            );
        }

        group.finish();
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(250))
        .measurement_time(Duration::from_secs(1))
        .sample_size(20);
    targets = benchmark_sample_serialization
}
criterion_main!(benches);
