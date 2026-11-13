// SPDX-License-Identifier: Apache-2.0

//! Benchmarks for Rego engine input and evaluation with synthetic context.

use std::{hint::black_box, time::Duration};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde::Serialize;
use weaver_checker::{Engine, FindingLevel, PolicyFinding, PolicyStage};
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
struct Input<'a> {
    sample: Sample<'a>,
    resource: Option<&'a Context>,
    instrumentation_scope: Option<&'a Context>,
}

#[derive(Serialize)]
struct Sample<'a> {
    span: Span<'a>,
}

#[derive(Serialize)]
struct Span<'a> {
    name: &'a str,
}

#[derive(Serialize)]
struct Context {
    attributes: Vec<Attribute>,
}

#[derive(Serialize)]
struct Attribute {
    name: String,
    value: String,
}

struct Fixture {
    name: &'static str,
    resource: Option<Context>,
    scope: Option<Context>,
}

impl Fixture {
    fn new(name: &'static str, resource: Option<usize>, scope: Option<usize>) -> Self {
        Self {
            name,
            resource: resource.map(|count| context("resource", count)),
            scope: scope.map(|count| context("scope", count)),
        }
    }

    fn input(&self, name: &'static str) -> Input<'_> {
        Input {
            sample: Sample {
                span: Span { name },
            },
            resource: self.resource.as_ref(),
            instrumentation_scope: self.scope.as_ref(),
        }
    }
}

fn context(kind: &str, count: usize) -> Context {
    Context {
        attributes: (0..count)
            .map(|index| Attribute {
                name: format!("benchmark.{kind}.attribute.{index:03}"),
                value: format!("benchmark-{kind}-value-{index:03}"),
            })
            .collect(),
    }
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
        .add_policy("rego_engine_benchmark.rego", BENCHMARK_POLICY)
        .expect("benchmark policy must compile");
    engine
}

fn preflight(fixtures: &[Fixture]) {
    let mut engine = benchmark_engine();

    for fixture in fixtures {
        engine
            .set_input(&fixture.input("benchmark.match"))
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

        engine
            .set_input(&fixture.input("benchmark.no_match"))
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

fn benchmark_rego_engine(c: &mut Criterion) {
    let fixtures = fixtures();
    preflight(&fixtures);

    for fixture in &fixtures {
        let mut group = c.benchmark_group(format!("rego_engine/{}", fixture.name));

        for batch_size in [1u64, 32, 256] {
            let _ = group.throughput(Throughput::Elements(batch_size));
            let mut set_input_engine = benchmark_engine();
            let _ = group.bench_function(
                BenchmarkId::new("set_input", format!("batch_{batch_size}")),
                |b| {
                    b.iter(|| {
                        for _ in 0..batch_size {
                            let input = fixture.input("benchmark.match");
                            let result = set_input_engine.set_input(black_box(&input));
                            black_box(result).expect("benchmark input must serialize");
                        }
                    });
                },
            );

            let mut evaluation_engine = benchmark_engine();
            let evaluation_input = fixture.input("benchmark.match");
            evaluation_engine
                .set_input(&evaluation_input)
                .expect("benchmark input must serialize");
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
            let _ = group.bench_function(
                BenchmarkId::new("set_input_and_check", format!("batch_{batch_size}")),
                |b| {
                    b.iter(|| {
                        for _ in 0..batch_size {
                            let input = fixture.input("benchmark.match");
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
    targets = benchmark_rego_engine
}
criterion_main!(benches);
