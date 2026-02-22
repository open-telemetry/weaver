# Entities

This document describes the entity semantic conventions.

## Namespace: `library`

### `otel.library`

Span attributes used by non-OTLP exporters to represent OpenTelemetry Scope's concepts.

| Property | Value |
|----------|-------|
| Stability | Stable |

#### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `otel.library.name` | `string` | Recommended | 
 |
| `otel.library.version` | `string` | Recommended | 
 |

## Namespace: `scope`

### `otel.scope`

Attributes used by non-OTLP exporters to represent OpenTelemetry Scope's concepts.

| Property | Value |
|----------|-------|
| Stability | Stable |

#### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `otel.scope.name` | `string` | Recommended | The name of the instrumentation scope - (`InstrumentationScope.Name` in OTLP).
 |
| `otel.scope.version` | `string` | Recommended | The version of the instrumentation scope - (`InstrumentationScope.Version` in OTLP).
 |

