# Schema transformations in Weaver, using OpenTelemetry Telemetry Policies as the transformation language

**Status:** design draft, for discussion

A **transformation file** describes how telemetry defined by one Weaver schema becomes telemetry defined by another. Weaver validates it against both schemas at check-in. It is written in OpenTelemetry Telemetry Policies ([OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md), [opentelemetry-proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865)) and follows whatever syntax they settle on. No new language.

## Scope

- **Source → target**, both Weaver schemas: the same schema at different versions (semconv 1.20.0 → 1.23.1), or different schemas (native gRPC → OTel RPC). Non-Weaver telemetry can be described as a Weaver schema first; it doesn't need to be published.
- **One direction per file.** Old → new and new → old are written separately, never derived from each other.
- **Lossy by design.** Only changes that can be converted correctly are covered. The rest stays in migration guides.
- **Out of scope:** general-purpose processing, deduplicating dual emission (`…/dup`).

---

## The format

A thin wrapper around a list of [telemetry policies](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md). The examples use the prototype's policy notation; the syntax will follow Telemetry Policies.

> [!TIP]
> **Worked example:** the [migration guides](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative) (HTTP, DB, RPC, K8s, code), GenAI and gRPC compatibility, in both directions: [`transformations` on the prototype branch](https://github.com/lmolkova/semantic-conventions/tree/transformations/transformations).

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

| Field | Required | Meaning |
|---|---|---|
| `source`, `target` | yes | Same shape as a Weaver manifest dependency. |
| `.schema_url` | yes | Identifies the schema and version. Must be a valid schema URL; doesn't have to be fetchable. |
| `.registry_path` | no | Where the schema files are ([vdir](https://github.com/open-telemetry/weaver/blob/main/docs/vdir.md): local dir, archive, Git repo, …). Default: resolved from `schema_url`. `.weaver.toml` schema URL overrides apply. |
| `policies` | yes | Telemetry Policies, unchanged. Without the wrapper they're a policy set the Collector or an SDK can run. |

**Policies.** One signal each (`trace`, `metric`, `log`, `entity`). All `matches` must hold: `equals`, `in`, `glob`, `exists`, `gt`/`gte`/`lt`/`lte`, `negate`. Operations: `rename` (optionally with type `convert`), `add` (typed; also sets signal names), `remove`, `redact`, `keep: false`. `metric_view` policies hold aggregation changes: dropping attributes and `scale` (unit change).

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

Distribution is out of scope: likely shipped with the resolved schema, but files can come from anywhere.

## Constraints

1. **Unambiguous.** Each rule has exactly one correct result for every record it matches. Changed meaning under the same name, ambiguous splits and data the source never captured are not rules.
2. **Target-safe.** Target-shaped telemetry passes through unchanged. Rules match on something only the source has.
3. **No conflicts.** Rules that can match the same record don't write the same field differently.

---

## Validation at check-in

Weaver resolves both schemas, applies the policies to the **source definitions** (metrics, spans, events, entities) and compares the result with the **target definitions**. It also applies them to the target definitions, which must come out unchanged. Value matchers "may match" attributes whose type or enum allows the value. Spans are matched by kind, name and attributes until span identity is on the wire.

| Check | Fails when |
|---|---|
| Names exist | a read name isn't in the source, or a written name (`to`, `add`) isn't in the target |
| Types match | a moved or added value's type differs from the target attribute's, without `convert` |
| Signals line up | a transformed metric's instrument or unit (after `scale`) differs from the target's |
| Values allowed | an added value isn't in the target attribute's enum |
| Required attributes | a target-required attribute isn't produced, and the gap isn't explicitly accepted in the file |
| Target-safe | a policy changes a target definition |
| No conflicts | overlapping policies write the same field differently |
| Units convertible | `scale` units have different dimensions |

Validation doesn't check coverage (not a goal) or real data.

## Why Telemetry Policies

They're designed to modify telemetry in the Collector and SDKs, so ingestion-time transformation comes for free: the file Weaver validates runs there as-is. One way to describe telemetry changes in OpenTelemetry, not a Weaver-only language.

---

## Future

1. **Dashboard and alert migration:** rewrite queries to also read the other version, or normalize to one version on the backend.
2. **Live check:** transform samples on the fly and validate them against the target schema.
3. **Migration docs** generated from transformation files.
4. **More operations** (value maps, copy, split/concat, prefix renames), contributed to Telemetry Policies, not Weaver.

## Alternatives considered

- **OTTL:** runs only in the Collector; the implementation is the spec.
- **Schema files v1:** renames only, wrong in places, being dropped by [OTEP #4815](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4815-semantic-conventions-schema-v2.md#schema-transformations), which lists richer transformations as an [open problem](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4815-semantic-conventions-schema-v2.md#schema-transformations-evolution). Deprecation metadata (`renamed_to`) still seeds simple rules.
- **Deprecated block per definition:** can't express both directions or external schemas, bloats the size-optimized resolved schema, and published versions can't change.
