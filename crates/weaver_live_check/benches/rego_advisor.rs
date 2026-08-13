// SPDX-License-Identifier: Apache-2.0

//! Benchmarks for the Rego advisor's shared-context input costs.

use std::{hint::black_box, rc::Rc, time::Duration};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde::Serialize;
use serde_json::Value;
use weaver_checker::{Engine, FindingLevel, PolicyFinding, PolicyStage};
use weaver_live_check::{
    sample_attribute::SampleAttribute, sample_instrumentation_scope::SampleInstrumentationScope,
    sample_resource::SampleResource, sample_span::SampleSpan, SampleRef,
};
use weaver_semconv::group::SpanKindSpec;

const BENCHMARK_POLICY: &str = r#"
package live_check_advice

import rego.v1

context_or_empty(context) := context if {
    context != null
}

context_or_empty(context) := {} if {
    context == null
}

context_attributes(context) := object.get(context_or_empty(context), "attributes", [])

deny contains {
    "id": "benchmark_match",
    "level": "information",
    "message": "benchmark match",
} if {
    sample := object.get(input, "sample", {})
    span := object.get(sample, "span", {})
    object.get(span, "name", "") == "benchmark.match"

    resource_attributes := context_attributes(object.get(input, "resource", null))
    scope_attributes := context_attributes(object.get(input, "instrumentation_scope", null))
    resource_attribute_count := count(resource_attributes)
    scope_attribute_count := count(scope_attributes)
    # These checks intentionally force both context traversals for the benchmark.
    resource_attribute_count >= 0
    scope_attribute_count >= 0
}
"#;

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
                version: "1.0.0".to_owned(),
                schema_url: "https://opentelemetry.io/schemas/1.0.0".to_owned(),
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

fn expected_finding() -> PolicyFinding {
    PolicyFinding {
        id: "benchmark_match".to_owned(),
        context: None,
        message: "benchmark match".to_owned(),
        level: FindingLevel::Information,
        signal_type: None,
        signal_name: None,
    }
}

fn benchmark_engine() -> Engine {
    let mut engine = Engine::new();
    let _ = engine
        .add_policy("rego_advisor_benchmark.rego", BENCHMARK_POLICY)
        .expect("benchmark policy must compile");
    engine
}

fn preflight(fixtures: &[Fixture]) {
    let mut engine = benchmark_engine();

    for fixture in fixtures {
        engine
            .set_input(&fixture.input())
            .unwrap_or_else(|error| panic!("{} input must serialize: {error}", fixture.name));
        let findings = engine
            .check(PolicyStage::LiveCheckAdvice)
            .unwrap_or_else(|error| panic!("{} policy check must succeed: {error}", fixture.name));
        assert_eq!(
            findings,
            vec![expected_finding()],
            "{} must produce the fixed benchmark finding",
            fixture.name
        );

        let mut nonmatching_span = fixture.span.clone();
        nonmatching_span.name = "benchmark.no_match".to_owned();
        engine
            .set_input(&input_for_span(&nonmatching_span))
            .unwrap_or_else(|error| {
                panic!("{} nonmatching input must serialize: {error}", fixture.name)
            });
        let findings = engine
            .check(PolicyStage::LiveCheckAdvice)
            .unwrap_or_else(|error| {
                panic!(
                    "{} nonmatching policy check must succeed: {error}",
                    fixture.name
                )
            });
        assert!(
            findings.is_empty(),
            "{} nonmatching span must produce no findings: {findings:?}",
            fixture.name
        );
    }
}

fn benchmark_rego_advisor(c: &mut Criterion) {
    let fixtures = fixtures();
    preflight(&fixtures);

    for fixture in &fixtures {
        let mut group = c.benchmark_group(format!("rego_advisor/{}", fixture.name));

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

            let mut set_input_engine = benchmark_engine();
            let _ = group.throughput(Throughput::Elements(batch_size));
            let _ = group.bench_function(
                BenchmarkId::new("set_input", format!("batch_{batch_size}")),
                |b| {
                    b.iter(|| {
                        for _ in 0..batch_size {
                            let input = fixture.input();
                            let result = set_input_engine.set_input(black_box(&input));
                            black_box(result).expect("benchmark input must serialize");
                        }
                    });
                },
            );

            let mut evaluation_engine = benchmark_engine();
            let evaluation_input = fixture.input();
            evaluation_engine
                .set_input(&evaluation_input)
                .expect("benchmark input must serialize");
            let _ = group.throughput(Throughput::Elements(batch_size));
            let _ = group.bench_function(
                BenchmarkId::new("policy_evaluation", format!("batch_{batch_size}")),
                |b| {
                    b.iter(|| {
                        for _ in 0..batch_size {
                            let findings = evaluation_engine
                                .check(PolicyStage::LiveCheckAdvice)
                                .expect("benchmark policy check must succeed");
                            let _ = black_box(findings);
                        }
                    });
                },
            );

            let mut combined_engine = benchmark_engine();
            let _ = group.throughput(Throughput::Elements(batch_size));
            let _ = group.bench_function(
                BenchmarkId::new("set_input_and_check", format!("batch_{batch_size}")),
                |b| {
                    b.iter(|| {
                        for _ in 0..batch_size {
                            let input = fixture.input();
                            combined_engine
                                .set_input(black_box(&input))
                                .expect("benchmark input must serialize");
                            let findings = combined_engine
                                .check(PolicyStage::LiveCheckAdvice)
                                .expect("benchmark policy check must succeed");
                            let _ = black_box(findings);
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
    targets = benchmark_rego_advisor
}
criterion_main!(benches);
