# Schema transformations in Weaver, using OpenTelemetry Telemetry Policies as the transformation language

**Status:** design draft, for discussion

A **transformation file** describes how telemetry defined by one Weaver schema becomes telemetry defined by another. Weaver validates it against both schemas at check-in. It is written in OpenTelemetry Telemetry Policies ([OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md), [opentelemetry-proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865)) and follows whatever syntax they settle on. 

## Scope

- **Source → target**, both Weaver schemas: the same schema at different versions (semconv 1.20.0 → 1.23.1), or different schemas (native gRPC → OTel RPC). Non-Weaver telemetry can be described as a Weaver schema first; it doesn't need to be published.
- **One direction per file.** Old → new and new → old are written separately, never derived from each other.
- **Lossy by design.** Transformations don't have to cover everything or be exact; approximate conversions are fine if they produce valid target telemetry. The rest stays in migration guides.
- **Out of scope:** general-purpose processing, deduplicating dual emission

---

## The format

A thin wrapper around a list of [telemetry policies](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md). The examples use the prototype's policy notation; the syntax will follow Telemetry Policies.

Policies are designed to modify telemetry in the Collector and SDKs, so export/ingestion-time transformation comes for free.

> [!TIP]
> **Example:** the [migration guides](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative) (HTTP, DB, RPC, K8s, code), GenAI and gRPC compatibility, in both directions: [`examples`](examples/).

```yaml
file_format: transform/2.0
source:
  schema_url: https://opentelemetry.io/schemas/1.20.0
target:
  schema_url: https://opentelemetry.io/schemas/1.23.1
policies:
  - id: http-span-common
    name: HTTP span attributes renamed
    trace:
      matches: [{span_attribute: [http.method], exists: true}]
      transform:
        rename:
          - {span_attribute: [http.status_code], to: http.response.status_code}
          - {span_attribute: [net.peer.name], to: server.address}
```

**Metric rename + unit change:**

```yaml
- id: http-server-duration
  name: http.server.duration becomes http.server.request.duration
  metric:
    matches:
      - {metric_field: name, equals: http.server.duration}
      - {datapoint_attribute: [http.method], exists: true}   # 1.20 only
    transform:
      rename: [{datapoint_attribute: [http.method], to: http.request.method}]
      add: [{metric_field: name, value: http.server.request.duration, upsert: true}]
- id: http-server-duration-view
  name: http.server.duration ms -> s
  metric_view:
    matches:
      - {metric_field: name, equals: http.server.duration}
      - {datapoint_attribute: [http.method], exists: true}
    view:
      # note: this might be invalid depending on instrument
      scale: [{metric_field: unit, from: ms, to: s}]
```

**Different schemas, unpublished source:**

```yaml
source:
  schema_url: https://example.com/schemas/grpc-native/1.0.0   # identifier only
  registry_path: ./grpc-native-model                           # local Weaver schema
target:
  schema_url: https://opentelemetry.io/schemas/1.45.0
policies:
  - id: grpc-client-call-duration
    name: grpc.client.call.duration becomes rpc.client.call.duration
    metric:
      matches: [{metric_field: name, equals: grpc.client.call.duration}]
      transform:
        rename: [{datapoint_attribute: [grpc.method], to: rpc.method}]
        add:
          - {metric_field: name, value: rpc.client.call.duration, upsert: true}
          - {datapoint_attribute: [rpc.system.name], value: grpc}
```

## Validation 

Weaver resolves both schemas, applies the policies to the **source definitions** (metrics, spans, events, entities) and compares the result with the **target definitions**. It also applies them to the target definitions, which must come out unchanged. Value matchers "may match" attributes whose type or enum allows the value. Spans are matched by kind, name and attributes until span type is on the wire.

Examples of checks:

- Signal ids and attribute keys exist in the source and target schemas.
- Signal properties match when a signal is renamed: span kind, metric instrument, unit after `scale`.
- Attributes after transformation have a compatible type and are referenced by the target signal.
- Added values are allowed by the target attribute (enum members).
- Required attributes are present on the target signal after transformation, unless the gap is explicitly accepted.
- Optional: all required and recommended target attributes can be populated.
- Target-shaped definitions come out unchanged.
- Policies that can match the same record don't write the same field differently.

File validation doesn't check real data (see Future).

---

## Usages

0. Telemetry transformation at ingestion time.
1. **Dashboard and alert migration:** rewrite queries to also read the other version, or normalize to one version on the backend.
2. **Live check:** transform samples on the fly and validate them against the target schema.
3. **Migration docs** generated from transformation files.

## Alternatives considered

- **OTTL:** runs only in the Collector; the implementation is the spec.
- **Schema files v1:** renames only, wrong in places, being dropped by [OTEP #4815](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4815-semantic-conventions-schema-v2.md#schema-transformations), which lists richer transformations as an [open problem](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4815-semantic-conventions-schema-v2.md#schema-transformations-evolution). Deprecation metadata (`renamed_to`) still seeds simple rules.
- **Deprecated block per definition:** can't express both directions or external schemas, bloats the size-optimized resolved schema, and published versions can't change.
