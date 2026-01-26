# Resources

This document describes the resource semantic conventions.

## Namespace: `library`

### `otel.library`

Span attributes used by non-OTLP exporters to represent OpenTelemetry Scope's concepts.

| Property | Value |
|----------|-------|
| Stability | Stable |

#### Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `otel.library.name` | `string` | No |  |
| `otel.library.version` | `string` | No |  |

## Namespace: `scope`

### `otel.scope`

Attributes used by non-OTLP exporters to represent OpenTelemetry Scope's concepts.

| Property | Value |
|----------|-------|
| Stability | Stable |

#### Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `otel.scope.name` | `string` | No | The name of the instrumentation scope - (`InstrumentationScope.Name` in OTLP). |
| `otel.scope.version` | `string` | No | The version of the instrumentation scope - (`InstrumentationScope.Version` in OTLP). |

