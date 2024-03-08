# Group `otel.scope` (resource)

## Brief

Attributes used by non-OTLP exporters to represent OpenTelemetry Scope's concepts.

prefix: otel.scope

## Attributes


### Attribute `otel.scope.name`

The name of the instrumentation scope - (`InstrumentationScope.Name` in OTLP).


- Requirement Level: Recommended

- Type: string
- Examples: [
    "io.opentelemetry.contrib.mongodb",
]

- Stability: Stable


### Attribute `otel.scope.version`

The version of the instrumentation scope - (`InstrumentationScope.Version` in OTLP).


- Requirement Level: Recommended

- Type: string
- Examples: [
    "1.0.0",
]

- Stability: Stable



## Provenance

Source: data/exporter.yaml

