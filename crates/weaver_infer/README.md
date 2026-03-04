# Weaver Infer

> Status: Experimental

Core inference logic for `weaver registry infer`.

This crate accumulates telemetry samples (resources, spans, metrics, and
log-based events) and converts them into a
[semantic convention](https://opentelemetry.io/docs/specs/semconv/) registry
file.

## Overview

The typical workflow is:

1. Create an `AccumulatedSamples` accumulator.
2. Feed it `Sample` values via `add_sample()` as they arrive.
3. When collection is done, call `to_semconv_spec()` to produce an
   `InferredRegistry` containing `GroupSpec` entries ready for YAML
   serialization.

```text
  OTLP / file / stdin
        │
        ▼
┌───────────────────┐
│ AccumulatedSamples│  ◄─── add_sample() called per sample
└────────┬──────────┘
         │  to_semconv_spec()
         ▼
┌───────────────────┐
│ InferredRegistry  │  ◄─── Vec<GroupSpec>, serializable to YAML
└───────────────────┘
```

## How it works

Each incoming `Sample` is dispatched by signal type:

| Sample variant    | Accumulated as                                        |
|-------------------|-------------------------------------------------------|
| `Resource`        | Resource attributes (single flat group)               |
| `Span`            | One span group per unique span name, with attributes and span events |
| `Metric`          | One metric group per unique metric name, with instrument, unit, and data-point attributes |
| `Log`             | One event group per unique event name, with attributes |

Attributes are deduplicated by name. When the same attribute appears multiple
times, its example values are collected (up to 5 unique examples per attribute).

The final `to_semconv_spec()` call converts the accumulated data into
`GroupSpec` entries following the semantic convention data model, sorted
alphabetically by attribute ID within each group.

## Architecture

This crate deliberately does **not** depend on OTLP protobuf types or CLI
frameworks. Those concerns live in the `weaver` binary (`src/registry/infer.rs`),
which converts raw OTLP messages into `Sample` values and passes them here. This
separation keeps the inference logic reusable and avoids circular dependencies.

```text
┌─────────────────────────────────┐
│  weaver binary                  │
│  src/registry/infer.rs          │
│  ┌───────────┐  ┌────────────┐  │
│  │ CLI (clap)│  │ OTLP gRPC  │  │
│  └─────┬─────┘  └─────┬──────┘  │
│        │              │         │
│        │   Sample values        │
│        │      │                 │
│        ▼      ▼                 │
│  ┌──────────────────────┐       │
│  │   weaver_infer crate │       │
│  │  (this crate)        │       │
│  └──────────────────────┘       │
└─────────────────────────────────┘
```
