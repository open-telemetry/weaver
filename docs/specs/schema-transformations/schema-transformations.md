# Schema transformations in Weaver, using OpenTelemetry Telemetry Policies as the transformation language

**Status:** draft

A **transformation file** describes how telemetry defined by one Weaver schema becomes telemetry defined by another. Weaver validates it against both schemas at check-in. It is written in OpenTelemetry Telemetry Policies ([OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md), [opentelemetry-proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865)) and follows whatever syntax they settle on. 

- [Scope](#scope)
- [The format](#the-format)
- [Validation](#validation)
- [Usages](#usages)
- [Alternatives considered](#alternatives-considered)
- [Appendix: case study of semconv migration guides](#appendix-case-study-of-semconv-migration-guides)
  - [HTTP](#http)
  - [RPC](#rpc)
  - [GenAI](#genai)
  - [K8s](#k8s)

## Scope

- **Source → target**, both Weaver schemas: the same schema at different versions (semconv 1.20.0 → 1.23.1), or different schemas (native gRPC → OTel RPC). Non-Weaver telemetry can be described as a Weaver schema first; it doesn't need to be published.
- **One direction per file.** Old → new and new → old are written separately, never derived from each other.
- **No chaining.** One file covers the whole migration, including names from intermediate releases.
- **Lossy by design.** Transformations don't have to cover everything or be exact; approximate conversions are fine if they produce valid target telemetry. The rest stays in migration guides.
- **Out of scope:** general-purpose processing, deduplicating dual emission

---

## The format

A source, a target and a list of [telemetry policies](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md). The examples use [proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865) notation.

The same policies are applied in two ways:

- **Data mode:** convert telemetry records, for example in a Collector processor or an SDK.
- **Schema mode:** apply the policies to the definitions in a resolved schema. Query rewriters, dashboard and alert migration tools, and validators use this mode. It never touches stored data: queries written against the target are rewritten to also read data in the source shape.

Both modes MUST agree: converting the data and querying it against the target gives the same result as running the rewritten query on the original data.

> [!TIP]
> **Example:** the [migration guides](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative) (HTTP, RPC, K8s), GenAI and gRPC compatibility, in both directions: [`examples`](examples/).

```yaml
file_format: transform/2.0
source:
  schema_url: https://opentelemetry.io/schemas/1.20.0
target:
  schema_url: https://opentelemetry.io/schemas/1.23.1
policies:
  - id: http-span-client
    name: HTTP client span attributes renamed
    trace_transform:
      matches:
        - target: {record_field: SPAN_RECORD_FIELD_KIND}
          predicate: {equals: CLIENT}
        - target: {span_attribute: [http.method]}
          predicate: {exists: {}}
      rename:
        - {from: {span_attribute: [http.status_code]}, to: http.response.status_code}
        - {from: {span_attribute: [net.peer.name]}, to: server.address}
```

**Metric rename:**

```yaml
- id: http-server-duration
  name: http.server.duration becomes http.server.request.duration
  metric_transform:
    matches:
      - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}
        predicate: {equals: http.server.duration}
      - target: {datapoint_attribute: [http.method]}   # 1.20 only
        predicate: {exists: {}}
    rename: [{from: {datapoint_attribute: [http.method]}, to: http.request.method}]
    add: [{target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}, value: http.server.request.duration, upsert: true}]
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
    metric_transform:
      matches:
        - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}
          predicate: {equals: grpc.client.call.duration}
      rename: [{from: {datapoint_attribute: [grpc.method]}, to: rpc.method}]
      add:
        - {target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}, value: rpc.client.call.duration, upsert: true}
        - {target: {datapoint_attribute: [rpc.system.name]}, value: grpc}
```

### Existing policy features used

From [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md) / [opentelemetry-proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865):

- **Policy types:** `log_transform`, `metric_filter`, `trace_sampling`.
- **Matcher targets:** `record_field`, `descriptor_field`, `span_attribute`, `datapoint_attribute`, `log_attribute`, `resource_attribute`.
- **Matcher predicates:** `exists`, `equals`, `regex`, `gt`/`gte`/`lt`/`lte`, `negate`.
- **Transforms:** `remove`, `redact`, `rename`, `add` (with `upsert`).

### Additional features needed

Needed on top of [proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865) for the migration guides (impact from the [appendix](#appendix-case-study-of-semconv-migration-guides)):

| Feature | Use | Impact |
|---|---|---|
| `trace_transform`, `metric_transform` policy types, same shape as `log_transform` | Any change on spans or metrics. [proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865) has transforms for logs only | High |
| `add` on `descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME` | Metric rename. [proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865) can match the name (`METRIC_DESCRIPTOR_FIELD_NAME`) but not write it | High |
| `map` | Change values: enum values, status codes (`rpc.system`, `rpc.grpc.status_code` ↔ `rpc.status_code`) | High |
| `convert` | Change type: `error.type` from an int status code, JSON-RPC error codes | High |
| `from` in `add` | Derive an attribute from other attributes or a span field: `error.type` from a status code, `rpc.service` + `rpc.method`, `grpc.target`, `rpc.method` from the span name | High |
| `split` | Fully-qualified `rpc.method` → `rpc.service` + `rpc.method` | High |
| Data point value as a field (`datapoint_field: DATAPOINT_FIELD_VALUE`) in matchers, `rename` and `map` | State in the value ↔ state in an attribute: K8s `k8s.pod.phase`, `k8s.node.condition_*` | High |
| `rename_prefix` | Templated keys: K8s `k8s.pod.labels.<key>` ↔ `k8s.pod.label.<key>` | Medium |
| `entity_transform` policy type, same shape as `log_transform` | Resource attribute renames: K8s `k8s.resource_quota.name` → `k8s.resourcequota.name`, `container.runtime` → `container.runtime.name` | Medium |
| `to_target` in `rename` | Move a resource attribute to data points: K8s `k8s.volume.name` | Medium |
| `add` on `record_field: SPAN_RECORD_FIELD_STATUS_CODE` | Set span status code (gRPC, MCP). Needs transforms on spans, [proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865) can only match it | Medium |
| `discard` | Drop attributes that `add` or `split` read: `rpc.service` after the `rpc.method` join, `code.namespace`, `db.name` | Low |

Transforms run in a fixed order ([OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md#current-stages)): `remove` → `redact` → `rename`/`rename_prefix` → `split` → `add` → `map` → `convert` → `discard`. Each step sees the output of the previous ones, so `map` changes a value `add` just wrote:

```yaml
add: [{target: {span_attribute: [error.type]}, from: {span_attribute: [rpc.grpc.status_code]}}]
map: [{target: {span_attribute: [error.type]}, values: {2: UNKNOWN, 4: DEADLINE_EXCEEDED}, unmapped: remove}]
```

### Rules

**Every policy MUST leave target-shaped data unchanged.** Match on something only the source has, like `http.method` in `http-server-duration` above. A meaning change under the same name (`server.address` on RPC spans), a reused name (`v8js.memory.heap.limit` in 1.42) or a unit change under the same name (`http.server.duration`) can't be a policy.

**Policies are independent.** Matchers see the input record, and the operations of all matching policies run together in the order above. Two policies that can match the same record MUST NOT apply the same operation to the same field with different parameters; [proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865) orders such writes instead. A value change goes in the policy that renames the key:

```yaml
trace_transform:
  matches:
    - target: {span_attribute: [rpc.system]}
      predicate: {exists: {}}
  rename: [{from: {span_attribute: [rpc.system]}, to: rpc.system.name}]
  map: [{target: {span_attribute: [rpc.system.name]}, values: {apache_dubbo: dubbo}, unmapped: keep}]
```

**`remove` on metrics only when series don't merge.** Pin the value with `equals`, otherwise leave the attribute in place:

```yaml
metric_transform:
  matches:
    - target: {datapoint_attribute: [gen_ai.token.type]}
      predicate: {equals: input}
  remove: [{target: {datapoint_attribute: [gen_ai.token.type]}}]
```

**Failures.** Data mode skips a policy it can't apply, as in [proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865). Schema mode MUST reject the queries it can't rewrite.

## Validation 

Weaver resolves both schemas, applies the policies to the **source definitions** (metrics, spans, events, entities) and compares the result with the **target definitions**. Value matchers "may match" attributes whose type or enum allows the value. Spans are matched by kind, name and attributes until span type is on the wire.

Examples of checks on schemas:

- Signal ids and attribute keys exist in the source and target schemas.
- Signal properties match when a signal is renamed: span kind, metric instrument, unit.
- Attributes after transformation have a compatible type and are referenced by the target signal.
- Added values are allowed by the target attribute (enum members).
- Required attributes are present on the target signal after transformation, unless the gap is explicitly accepted.
- Optional: all required and recommended target attributes can be populated.
- Target-shaped definitions come out unchanged.
- Policies that can match the same record don't write the same field differently.

---

## Usages

0. **Telemetry transformation at ingestion time**
1. **Dashboard and alert migration:** rewrite queries to also read the other version, or normalize to one version on the backend.
2. **Live check:** transform samples on the fly and validate them against the target schema.
3. **Migration docs** generated from transformation files.

## Alternatives considered

- **OTTL:** runs only in the Collector; the implementation is the spec.
- **Schema files v1:** renames only, wrong in places, being dropped by [OTEP #4815](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4815-semantic-conventions-schema-v2.md#schema-transformations), which lists richer transformations as an [open problem](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4815-semantic-conventions-schema-v2.md#schema-transformations-evolution). Deprecation metadata (`renamed_to`) still seeds simple rules.
- **Deprecated block in definition schema:** can't express both directions or external schemas, bloats the size-optimized resolved schema, and published versions can't change.

---

## Appendix: case study of semconv migration guides

Types of changes made in the HTTP, RPC and K8s [migration guides](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative), GenAI and gRPC compatibility, see [`examples`](examples/) for details.

**Summary**: High-impact changes that can't be transformed precisely:
- **Unit change** on duration histograms (HTTP, RPC, both directions): stored data can only be rescaled at query time. Filters and alerts on bucket bounds (`le`) break.
- **`grpc.target` ↔ `server.address` + `server.port`** (both directions): native targets may have a scheme (`dns:///host:443`) or no host:port at all.
- **HTTP `error.type` from `http.status_code`** (1.20.0 → 1.23.1): failures without a status code have no source data.
- **K8s state metrics** (`k8s.node.condition_*`, `k8s.pod.phase`, both directions): the old metric encodes state in the value, the new one in an attribute. Approximate: the `0`-valued series for the other states can't be created.

**Legend**:
- **Existing**: [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md) / [#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865) as is.
- **Reverse** (new → old): the user emits new telemetry and has old dashboards and alerts. Changes that break nothing are left out: new signals and attributes with no old counterpart, signals removed with no replacement, span names.
- **Possible**:
  - ✅ precise and unambiguous.
  - ⚠️ approximate or slightly ambiguous.
  - ❌ not possible.
- **Impact**: what breaks on typical dashboards and alerts if the change isn't handled.
  - **High**: golden signals (duration, throughput, error rate), metric names, and attributes that dashboards group or filter by.
  - **Medium**: less common breakdowns and filters.
  - **Low**: custom queries only (span-only, opt-in, or niche values).

### HTTP

#### 1.20.0 → 1.23.1

| Type of change | Examples | Existing policy functions | How to transform | Possible | Impact |
|---|---|---|---|---|---|
| Attribute rename | `http.status_code` → `http.response.status_code`, `net.peer.name` → `server.address` | Yes: `rename`:<pre>rename:<br>  - from: {span_attribute: [http.status_code]}<br>    to: http.response.status_code</pre> | - | ✅ | High |
| Metric rename | `http.server.duration` → `http.server.request.duration` | No | New field: metric name in `add`:<pre>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: http.server.request.duration<br>    upsert: true</pre> | ✅ | High |
| Unit change | `http.server.duration` ms → s | No | Query time only: multiply results (sums, averages, quantiles) by 0.001. Filters and alerts on bucket bounds (`le`) can't be rewritten | ⚠️ | High |
| Attribute dropped from metric | `net.host.name`, `net.host.port` on `http.server.duration` (`server.address`/`server.port` are opt-in on the stable server metric), `net.sock.peer.addr` on `http.client.duration` | No | Not needed: aggregation over target attributes ignores the extra one | ✅ | Low |
| Error attribute derived | `error.type` from `http.status_code` (`>=500` on server, `>=400` on client), on spans and duration metrics. Failures without a status code have no source data | Partially: `gte` matcher:<pre>matches:<br>  - target: {record_field: SPAN_RECORD_FIELD_KIND}<br>    predicate: {equals: SERVER}<br>  - target: {span_attribute: [http.status_code]}<br>    predicate: {gte: 500}</pre> | New field: `from` in `add`, new policy function: `convert`:<pre>matches:<br>  - target: {record_field: SPAN_RECORD_FIELD_KIND}<br>    predicate: {equals: SERVER}<br>  - target: {span_attribute: [http.status_code]}<br>    predicate: {gte: 500}<br>add:<br>  - target: {span_attribute: [error.type]}<br>    from: {span_attribute: [http.status_code]}<br>convert:<br>  - target: {span_attribute: [error.type]}<br>    type: string</pre> | ⚠️ | High |
| Enum value change | `net.protocol.version` `2.0` → `network.protocol.version` `2` | Partially: one policy per value:<pre>matches:<br>  - target: {span_attribute: [net.protocol.version]}<br>    predicate: {equals: "2.0"}<br>rename:<br>  - from: {span_attribute: [net.protocol.version]}<br>    to: network.protocol.version<br>add:<br>  - target: {span_attribute: [network.protocol.version]}<br>    value: "2"<br>    upsert: true</pre> | New policy function: `map`:<pre>rename:<br>  - from: {span_attribute: [net.protocol.version]}<br>    to: network.protocol.version<br>map:<br>  - target: {span_attribute: [network.protocol.version]}<br>    values: {"2.0": "2", "3.0": "3"}</pre> | ✅ | Medium |
| Value normalization | Unknown `http.method` → `_OTHER` + `http.request.method_original` | Yes: `regex` + `negate`, `rename`, `add`:<pre>matches:<br>  - target: {span_attribute: [http.method]}<br>    predicate: {regex: "^(GET|HEAD|POST|PUT|DELETE|CONNECT|OPTIONS|TRACE|PATCH)$"}<br>    negate: true<br>rename:<br>  - from: {span_attribute: [http.method]}<br>    to: http.request.method_original<br>add:<br>  - target: {span_attribute: [http.request.method]}<br>    value: _OTHER</pre> | - | ✅ | Medium |
| Intermediate release names | 1.21 `server.socket.*` → `network.peer.*` | Yes: `rename`:<pre>rename:<br>  - from: {span_attribute: [server.socket.address]}<br>    to: network.peer.address</pre> | - | ✅ | Low |
| Split one attribute into two | `http.target` → `url.path` + `url.query` | No | New policy function: `split`:<pre>split:<br>  - from: {span_attribute: [http.target]}<br>    separator: "?"<br>    to: [url.path, url.query]</pre> | ✅ | Low |
| Fallback value | `client.address` from `net.sock.peer.addr` | No | New field: `from` in `add`:<pre>add:<br>  - target: {span_attribute: [client.address]}<br>    from: {span_attribute: [net.sock.peer.addr]}</pre> | ✅ | Low |
| Templated attribute keys | `http.request.header.<key>` | No | New policy function: `rename_prefix`:<pre>rename_prefix:<br>  - from: {span_attribute: [http.request.header.]}<br>    to: http.request.header.<br>    replace: {"_": "-"}</pre> | ✅ | Low |

#### 1.23.1 → 1.20.0

| Type of change | Examples | Existing policy functions | How to transform | Possible | Impact |
|---|---|---|---|---|---|
| Attribute rename | `http.request.method` → `http.method`, `http.response.status_code` → `http.status_code`, `url.scheme` → `http.scheme` | Yes: `rename`:<pre>rename:<br>  - from: {span_attribute: [http.request.method]}<br>    to: http.method</pre> | - | ✅ | High |
| Conditional rename | `server.address` → `net.peer.name` on client, `net.host.name` on server | Yes: `rename` with matchers:<pre>matches:<br>  - target: {record_field: SPAN_RECORD_FIELD_KIND}<br>    predicate: {equals: CLIENT}<br>rename:<br>  - from: {span_attribute: [server.address]}<br>    to: net.peer.name</pre> | - | ✅ | High |
| Metric rename | `http.server.request.duration` → `http.server.duration` | No | New field: metric name in `add`:<pre>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: http.server.duration<br>    upsert: true</pre> | ✅ | High |
| Unit change | `http.server.request.duration` s → ms | No | Query time only: multiply results (sums, averages, quantiles) by 1000. Filters and alerts on bucket bounds (`le`) can't be rewritten | ⚠️ | High |
| Attribute missing on metric | `net.host.name` (Required in 1.20), `net.host.port` on `http.server.duration`: `server.address`/`server.port` are opt-in on the stable server metric. `net.sock.peer.addr` on `http.client.duration` | No | Impossible | ❌ | Medium |
| Attribute dropped from metric | `error.type` on duration metrics | No | Not needed: aggregation over target attributes ignores the extra one | ✅ | Low |
| Enum value change | `network.protocol.version` `2` → `net.protocol.version` `2.0` | Partially: one policy per value:<pre>matches:<br>  - target: {span_attribute: [network.protocol.version]}<br>    predicate: {equals: "2"}<br>rename:<br>  - from: {span_attribute: [network.protocol.version]}<br>    to: net.protocol.version<br>add:<br>  - target: {span_attribute: [net.protocol.version]}<br>    value: "2.0"<br>    upsert: true</pre> | New policy function: `map`:<pre>rename:<br>  - from: {span_attribute: [network.protocol.version]}<br>    to: net.protocol.version<br>map:<br>  - target: {span_attribute: [net.protocol.version]}<br>    values: {"2": "2.0", "3": "3.0"}</pre> | ✅ | Medium |
| Default value derived | Absent `network.protocol.name` with a version set → `net.protocol.name` `http` | Yes: `exists` matcher, `add`:<pre>matches:<br>  - target: {span_attribute: [network.protocol.name]}<br>    predicate: {exists: {}}<br>    negate: true<br>  - target: {span_attribute: [network.protocol.version]}<br>    predicate: {exists: {}}<br>add:<br>  - target: {span_attribute: [net.protocol.name]}<br>    value: http</pre> | - | ✅ | Low |
| Value normalization reverted | `http.request.method` `_OTHER` → original `http.method` | Partially: spans only, from `http.request.method_original`:<pre>matches:<br>  - target: {span_attribute: [http.request.method]}<br>    predicate: {equals: _OTHER}<br>remove:<br>  - target: {span_attribute: [http.request.method]}<br>rename:<br>  - from: {span_attribute: [http.request.method_original]}<br>    to: http.method</pre> | Impossible on metrics: the original method isn't there | ⚠️ | Low |
| Concatenate attributes | `url.path` + `url.query` → `http.target` | No | New field: `from` in `add`:<pre>add:<br>  - target: {span_attribute: [http.target]}<br>    from: [{span_attribute: [url.path]}, {span_attribute: [url.query]}]<br>    separator: "?"</pre> | ✅ | Low |
| Templated attribute keys | `http.request.header.<key>` | No | New policy function: `rename_prefix`:<pre>rename_prefix:<br>  - from: {span_attribute: [http.request.header.]}<br>    to: http.request.header.<br>    replace: {"-": "_"}</pre> | ✅ | Low |

### RPC

#### OTel semconv 1.37.0 → 1.45.0

| Type of change | Examples | Existing policy functions | How to transform | Possible | Impact |
|---|---|---|---|---|---|
| Attribute rename | `rpc.system` → `rpc.system.name`, `rpc.connect_rpc.error_code` → `rpc.status_code`, 1.39-1.44 `rpc.response.status_code` → `rpc.status_code` | Yes: `rename`:<pre>rename:<br>  - from: {span_attribute: [rpc.system]}<br>    to: rpc.system.name</pre> | - | ✅ | High |
| Metric rename | `rpc.server.duration` → `rpc.server.call.duration` | No | New field: metric name in `add`:<pre>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: rpc.server.call.duration<br>    upsert: true</pre> | ✅ | High |
| Unit change | `rpc.server.duration` ms → s | No | Query time only: multiply results (sums, averages, quantiles) by 0.001. Filters and alerts on bucket bounds (`le`) can't be rewritten | ⚠️ | High |
| Attribute dropped from metric | `network.transport`, `network.type`, `server.address`/`server.port` on `rpc.server.duration` (opt-in on the stable server metric) | No | Not needed: aggregation over target attributes ignores the extra one | ✅ | Low |
| Enum value change | `rpc.system` `apache_dubbo` → `dubbo`, `connect_rpc` → `connectrpc` | Partially: one policy per value:<pre>matches:<br>  - target: {span_attribute: [rpc.system]}<br>    predicate: {equals: apache_dubbo}<br>rename:<br>  - from: {span_attribute: [rpc.system]}<br>    to: rpc.system.name<br>add:<br>  - target: {span_attribute: [rpc.system.name]}<br>    value: dubbo<br>    upsert: true</pre> | New policy function: `map`:<pre>rename:<br>  - from: {span_attribute: [rpc.system]}<br>    to: rpc.system.name<br>map:<br>  - target: {span_attribute: [rpc.system.name]}<br>    values:<br>      apache_dubbo: dubbo<br>      connect_rpc: connectrpc</pre> | ✅ | High |
| Status code type and value change | `rpc.grpc.status_code` `0`..`16` → `rpc.status_code` `OK`..`UNAUTHENTICATED`, `rpc.jsonrpc.error_code` int → string | Partially: one policy per value (17 for gRPC):<pre>matches:<br>  - target: {span_attribute: [rpc.grpc.status_code]}<br>    predicate: {equals: 0}<br>rename:<br>  - from: {span_attribute: [rpc.grpc.status_code]}<br>    to: rpc.status_code<br>add:<br>  - target: {span_attribute: [rpc.status_code]}<br>    value: OK<br>    upsert: true</pre> | New policy function: `map`:<pre>rename:<br>  - from: {span_attribute: [rpc.grpc.status_code]}<br>    to: rpc.status_code<br>map:<br>  - target: {span_attribute: [rpc.status_code]}<br>    values: {0: OK, 1: CANCELLED}</pre>New policy function: `convert`:<pre>rename:<br>  - from: {span_attribute: [rpc.jsonrpc.error_code]}<br>    to: rpc.status_code<br>convert:<br>  - target: {span_attribute: [rpc.status_code]}<br>    type: string</pre> | ✅ | High |
| Error attribute derived | `error.type` from status code | No | New field: `from` in `add`, new policy function: `map`:<pre>add:<br>  - target: {span_attribute: [error.type]}<br>    from: {span_attribute: [rpc.grpc.status_code]}<br>map:<br>  - target: {span_attribute: [error.type]}<br>    values: {2: UNKNOWN, 4: DEADLINE_EXCEEDED}<br>    unmapped: remove</pre> | ✅ | High |
| Concatenate attributes | `rpc.service` + `rpc.method` → fully-qualified `rpc.method` | No | New field: `from` in `add`:<pre>add:<br>  - target: {span_attribute: [rpc.method]}<br>    from: [{span_attribute: [rpc.service]}, {span_attribute: [rpc.method]}]<br>    separator: /<br>    upsert: true</pre> | ✅ | High |
| Templated attribute keys | `rpc.grpc.request.metadata.<key>` → `rpc.request.header.<key>` | No | New policy function: `rename_prefix`:<pre>rename_prefix:<br>  - from: {span_attribute: [rpc.grpc.request.metadata.]}<br>    to: rpc.request.header.</pre> | ✅ | Low |
| Attribute split by unknown criteria | `rpc.grpc.response.metadata.<key>` → `rpc.response.header.<key>` or `rpc.response.trailer.<key>`; old data doesn't say which | No | New policy function: `rename_prefix`, everything mapped to headers (approximate):<pre>rename_prefix:<br>  - from: {span_attribute: [rpc.grpc.response.metadata.]}<br>    to: rpc.response.header.</pre> | ⚠️ | Low |
| Attribute moved to span field | `rpc.jsonrpc.error_message` → span status description | No | New field: `to_target` in `rename`:<pre>rename:<br>  - from: {span_attribute: [rpc.jsonrpc.error_message]}<br>    to_target: {record_field: SPAN_RECORD_FIELD_STATUS_MESSAGE}</pre> | ✅ | Low |
| Move between signals | `rpc.message` and exception span events → log events | No | - | ❌ | Low |

#### OTel semconv 1.45.0 → 1.37.0

| Type of change | Examples | Existing policy functions | How to transform | Possible | Impact |
|---|---|---|---|---|---|
| Attribute rename | `rpc.system.name` → `rpc.system`, `jsonrpc.request.id` → `rpc.jsonrpc.request_id`, `jsonrpc.protocol.version` → `rpc.jsonrpc.version` | Yes: `rename`:<pre>rename:<br>  - from: {span_attribute: [rpc.system.name]}<br>    to: rpc.system</pre> | - | ✅ | High |
| Metric rename | `rpc.server.call.duration` → `rpc.server.duration` | No | New field: metric name in `add`:<pre>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: rpc.server.duration<br>    upsert: true</pre> | ✅ | High |
| Unit change | `rpc.server.call.duration` s → ms | No | Query time only: multiply results (sums, averages, quantiles) by 1000. Filters and alerts on bucket bounds (`le`) can't be rewritten | ⚠️ | High |
| Enum value change | `rpc.system.name` `dubbo` → `apache_dubbo`, `connectrpc` → `connect_rpc` | Partially: one policy per value:<pre>matches:<br>  - target: {span_attribute: [rpc.system.name]}<br>    predicate: {equals: dubbo}<br>rename:<br>  - from: {span_attribute: [rpc.system.name]}<br>    to: rpc.system<br>add:<br>  - target: {span_attribute: [rpc.system]}<br>    value: apache_dubbo<br>    upsert: true</pre> | New policy function: `map`:<pre>rename:<br>  - from: {span_attribute: [rpc.system.name]}<br>    to: rpc.system<br>map:<br>  - target: {span_attribute: [rpc.system]}<br>    values:<br>      dubbo: apache_dubbo<br>      connectrpc: connect_rpc</pre> | ✅ | High |
| Status code type and value change | `rpc.status_code` → gRPC `rpc.grpc.status_code` `OK`..`UNAUTHENTICATED` → `0`..`16`, Connect `rpc.connect_rpc.error_code` `DEADLINE_EXCEEDED` → `deadline_exceeded` (errors only), JSON-RPC `rpc.jsonrpc.error_code` string → int. Spans and metrics | Partially: one policy per value (17 for gRPC):<pre>matches:<br>  - target: {datapoint_attribute: [rpc.system.name]}<br>    predicate: {equals: grpc}<br>  - target: {datapoint_attribute: [rpc.status_code]}<br>    predicate: {equals: OK}<br>rename:<br>  - from: {datapoint_attribute: [rpc.status_code]}<br>    to: rpc.grpc.status_code<br>add:<br>  - target: {datapoint_attribute: [rpc.grpc.status_code]}<br>    value: 0<br>    upsert: true</pre> | New policy function: `map`:<pre>rename:<br>  - from: {datapoint_attribute: [rpc.status_code]}<br>    to: rpc.grpc.status_code<br>map:<br>  - target: {datapoint_attribute: [rpc.grpc.status_code]}<br>    values: {OK: 0, CANCELLED: 1}</pre>New policy function: `convert`:<pre>rename:<br>  - from: {span_attribute: [rpc.status_code]}<br>    to: rpc.jsonrpc.error_code<br>convert:<br>  - target: {span_attribute: [rpc.jsonrpc.error_code]}<br>    type: int</pre> | ✅ | High |
| Split one attribute into two | Fully-qualified `rpc.method` → `rpc.service` + `rpc.method` | No | New policy function: `split`:<pre>split:<br>  - from: {datapoint_attribute: [rpc.method]}<br>    separator: /<br>    to: [rpc.service, rpc.method]</pre> | ✅ | High |
| Attribute missing on metric | `network.transport`, `network.type`, `server.address`/`server.port` on `rpc.server.duration` (opt-in on the stable server metric) | No | Impossible | ❌ | Low |
| Attribute dropped from metric | `error.type` on duration metrics | No | Not needed: aggregation over target attributes ignores the extra one | ✅ | Low |
| Value normalization reverted | `rpc.method` `_OTHER` → original `rpc.method` | Partially: spans only, from `rpc.method_original`:<pre>matches:<br>  - target: {span_attribute: [rpc.method]}<br>    predicate: {equals: _OTHER}<br>remove:<br>  - target: {span_attribute: [rpc.method]}<br>rename:<br>  - from: {span_attribute: [rpc.method_original]}<br>    to: rpc.method</pre> | Impossible on metrics: the original method isn't there | ⚠️ | Low |
| Templated attribute keys | `rpc.request.header.<key>` → `rpc.grpc.request.metadata.<key>` or `rpc.connect_rpc.request.metadata.<key>` | No | New policy function: `rename_prefix`:<pre>matches:<br>  - target: {span_attribute: [rpc.system.name]}<br>    predicate: {equals: grpc}<br>rename_prefix:<br>  - from: {span_attribute: [rpc.request.header.]}<br>    to: rpc.grpc.request.metadata.</pre> | ✅ | Low |
| Merge two attributes into one | `rpc.response.header.<key>` and `rpc.response.trailer.<key>` → `rpc.grpc.response.metadata.<key>` | No | New policy function: `rename_prefix`:<pre>rename_prefix:<br>  - from: {span_attribute: [rpc.response.header.]}<br>    to: rpc.grpc.response.metadata.<br>  - from: {span_attribute: [rpc.response.trailer.]}<br>    to: rpc.grpc.response.metadata.</pre> | ✅ | Low |
| Move between signals | Exception log events → exception span events | No | - | ❌ | Low |

#### Native gRPC (A66/A72) → OTel semconv

| Type of change | Examples | Existing policy functions | How to transform | Possible | Impact |
|---|---|---|---|---|---|
| Metric rename | `grpc.client.call.duration` → `rpc.client.call.duration` | No | New field: metric name in `add`:<pre>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: rpc.client.call.duration<br>    upsert: true</pre> | ✅ | High |
| Attribute rename | `grpc.method` → `rpc.method`, `grpc.status` → `rpc.status_code` | Yes: `rename`:<pre>rename:<br>  - from: {datapoint_attribute: [grpc.method]}<br>    to: rpc.method</pre> | - | ✅ | High |
| Constant attribute added | `rpc.system.name: grpc` | Yes: `add`:<pre>add:<br>  - target: {datapoint_attribute: [rpc.system.name]}<br>    value: grpc</pre> | - | ✅ | High |
| Single value change | `grpc.method` `other` → `rpc.method` `_OTHER` | Yes: `rename` + `add` with `upsert`:<pre>matches:<br>  - target: {datapoint_attribute: [grpc.method]}<br>    predicate: {equals: other}<br>rename:<br>  - from: {datapoint_attribute: [grpc.method]}<br>    to: rpc.method<br>add:<br>  - target: {datapoint_attribute: [rpc.method]}<br>    value: _OTHER<br>    upsert: true</pre> | - | ✅ | Medium |
| Error attribute derived | `error.type` from `grpc.status` (all non-OK codes on client, 6 codes on server) | Partially: one policy per code:<pre>matches:<br>  - target: {datapoint_attribute: [grpc.status]}<br>    predicate: {equals: CANCELLED}<br>add:<br>  - target: {datapoint_attribute: [error.type]}<br>    value: CANCELLED</pre> | New field: `from` in `add`:<pre>matches:<br>  - target: {datapoint_attribute: [grpc.status]}<br>    predicate: {equals: OK}<br>    negate: true<br>add:<br>  - target: {datapoint_attribute: [error.type]}<br>    from: {datapoint_attribute: [grpc.status]}</pre> | ✅ | High |
| Attribute parsed into two | `grpc.target` → `server.address` + `server.port` | Partially: `rename` as is (approximate):<pre>rename:<br>  - from: {datapoint_attribute: [grpc.target]}<br>    to: server.address</pre> | Parsing is not a goal (some targets, like `xds:///name` or `unix:path`, have no host:port) | ⚠️ | High |
| Attribute derived from span field | `rpc.method` from span name, `rpc.status_code` from status description | No | New field: `from` in `add`, reading a span field:<pre>add:<br>  - target: {span_attribute: [rpc.method]}<br>    from: {record_field: SPAN_RECORD_FIELD_NAME}</pre> | ✅ | Medium |
| Span status change | OTel sets `ERROR` only for some server codes | No | New: span transforms, `add` on `record_field`:<pre>add:<br>  - target: {record_field: SPAN_RECORD_FIELD_STATUS_CODE}<br>    value: UNSET<br>    upsert: true</pre> | ✅ | Medium |

#### OTel semconv → native gRPC (A66/A72)

| Type of change | Examples | Existing policy functions | How to transform | Possible | Impact |
|---|---|---|---|---|---|
| Metric rename | `rpc.client.call.duration` → `grpc.client.call.duration` | No | New field: metric name in `add`:<pre>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: grpc.client.call.duration<br>    upsert: true</pre> | ✅ | High |
| Attribute rename | `rpc.method` → `grpc.method`, `rpc.status_code` → `grpc.status` | Yes: `rename`:<pre>rename:<br>  - from: {datapoint_attribute: [rpc.method]}<br>    to: grpc.method</pre> | - | ✅ | High |
| Concatenate attributes | `server.address` + `server.port` → `grpc.target` | No | New field: `from` in `add`, approximate (native targets carry a scheme, e.g. `dns:///host:443`, or have no host:port):<pre>add:<br>  - target: {datapoint_attribute: [grpc.target]}<br>    from: [{datapoint_attribute: [server.address]}, {datapoint_attribute: [server.port]}]<br>    separator: ":"</pre> | ⚠️ | High |
| Single value change | `rpc.method` `_OTHER` → `grpc.method` `other` | Yes: `rename` + `add` with `upsert`:<pre>matches:<br>  - target: {datapoint_attribute: [rpc.method]}<br>    predicate: {equals: _OTHER}<br>rename:<br>  - from: {datapoint_attribute: [rpc.method]}<br>    to: grpc.method<br>add:<br>  - target: {datapoint_attribute: [grpc.method]}<br>    value: other<br>    upsert: true</pre> | - | ✅ | Medium |
| Attribute dropped from metric | `rpc.system.name`, `error.type` | No | Not needed: aggregation over target attributes ignores the extra one | ✅ | Low |
| Span status change | gRPC sets `ERROR` for every non-`OK` `rpc.status_code` | No | New: span transforms, `add` on `record_field`:<pre>matches:<br>  - target: {span_attribute: [rpc.status_code]}<br>    predicate: {equals: OK}<br>    negate: true<br>add:<br>  - target: {record_field: SPAN_RECORD_FIELD_STATUS_CODE}<br>    value: ERROR<br>    upsert: true</pre> | ✅ | Medium |

### GenAI

#### 1.41.0 → gen-ai-dev 1.42.0-dev

| Type of change | Examples | Existing policy functions | How to transform | Possible | Impact |
|---|---|---|---|---|---|
| Metric split by attribute value | `gen_ai.client.token.usage` with `gen_ai.token.type=input` and an inference `gen_ai.operation.name` (`chat`, `generate_content`, `text_completion`) → `gen_ai.client.inference.operation.input_tokens`. Token usage of other operations (e.g. embeddings) stays as is | Partially: `regex` matcher, `remove` of the pinned `gen_ai.token.type`. The metric name can't be written | New field: metric name in `add`:<pre>matches:<br>  - target: {datapoint_attribute: [gen_ai.token.type]}<br>    predicate: {equals: input}<br>  - target: {datapoint_attribute: [gen_ai.operation.name]}<br>    predicate: {regex: "^(chat|generate_content|text_completion)$"}<br>remove:<br>  - target: {datapoint_attribute: [gen_ai.token.type]}<br>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: gen_ai.client.inference.operation.input_tokens<br>    upsert: true</pre> | ✅ | High |
| Metric rename by attribute value | `gen_ai.client.operation.duration` for inference operations → `gen_ai.client.inference.duration` | Partially: `regex` matcher. The metric name can't be written | New field: metric name in `add`:<pre>matches:<br>  - target: {datapoint_attribute: [gen_ai.operation.name]}<br>    predicate: {regex: "^(chat|generate_content|text_completion)$"}<br>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: gen_ai.client.inference.duration<br>    upsert: true</pre> | ✅ | High |
| Metric rename | `gen_ai.client.operation.time_to_first_chunk` → `gen_ai.client.inference.time_to_first_chunk`, `gen_ai.client.operation.time_per_output_chunk` → `gen_ai.client.inference.time_per_output_chunk` | No | New field: metric name in `add`:<pre>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: gen_ai.client.inference.time_to_first_chunk<br>    upsert: true</pre> | ✅ | Medium |
| Error attribute removed for some values | MCP `error.type` for client-side JSON-RPC errors | Yes: `remove` (`rpc.response.status_code` keeps series distinct):<pre>matches:<br>  - target: {datapoint_attribute: [error.type]}<br>    predicate: {regex: "^(-32700|-32600)$"}<br>remove:<br>  - target: {datapoint_attribute: [error.type]}</pre> | - | ✅ | Medium |
| Attribute rename | `gen_ai.usage.cache_creation.input_tokens` → `gen_ai.usage.cache_write.input_tokens` (spans, events) | Yes: `rename`:<pre>rename:<br>  - from: {span_attribute: [gen_ai.usage.cache_creation.input_tokens]}<br>    to: gen_ai.usage.cache_write.input_tokens</pre> | - | ✅ | Low |
| Attribute type change | `gen_ai.request.top_k` double → int | No | New policy function: `convert`:<pre>convert:<br>  - target: {span_attribute: [gen_ai.request.top_k]}<br>    type: int</pre> | ✅ | Low |
| Conditional rename | `gen_ai.request.top_k` → `gen_ai.retrieval.top_k` on retrieval spans | Yes: `rename` with matchers:<pre>matches:<br>  - target: {span_attribute: [gen_ai.operation.name]}<br>    predicate: {equals: retrieval}<br>rename:<br>  - from: {span_attribute: [gen_ai.request.top_k]}<br>    to: gen_ai.retrieval.top_k</pre> | New policy function: `convert`:<pre>rename:<br>  - from: {span_attribute: [gen_ai.request.top_k]}<br>    to: gen_ai.retrieval.top_k<br>convert:<br>  - target: {span_attribute: [gen_ai.retrieval.top_k]}<br>    type: int</pre> | ✅ | Low |
| Attributes removed from some spans | `gen_ai.agent.id`, `gen_ai.provider.name` on internal `invoke_agent` spans | Yes: `remove`:<pre>remove:<br>  - target: {span_attribute: [gen_ai.agent.id]}</pre> | - | ✅ | Low |
| Span status change | MCP server spans for client-side errors | No | New: span transforms, `add` on `record_field`:<pre>add:<br>  - target: {record_field: SPAN_RECORD_FIELD_STATUS_CODE}<br>    value: UNSET<br>    upsert: true</pre> | ✅ | Medium |
| JSON content edits | `gen_ai.system_instructions` text-only, `finish_reason` back-fill | No | - | ❌ | Low |

#### gen-ai-dev 1.42.0-dev → 1.41.0

| Type of change | Examples | Existing policy functions | How to transform | Possible | Impact |
|---|---|---|---|---|---|
| Metrics merged by attribute value | `gen_ai.client.inference.operation.input_tokens` → `gen_ai.client.token.usage` with `gen_ai.token.type=input`, same for `output_tokens` | Partially: `add` on the attribute. The metric name can't be written | New field: metric name in `add`:<pre>matches:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    predicate: {equals: gen_ai.client.inference.operation.input_tokens}<br>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: gen_ai.client.token.usage<br>    upsert: true<br>  - target: {datapoint_attribute: [gen_ai.token.type]}<br>    value: input</pre> | ✅ | High |
| Metric rename | `gen_ai.client.inference.duration` → `gen_ai.client.operation.duration` | No | New field: metric name in `add`:<pre>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: gen_ai.client.operation.duration<br>    upsert: true</pre> | ✅ | High |
| Metric rename | `gen_ai.client.inference.time_to_first_chunk` → `gen_ai.client.operation.time_to_first_chunk`, `gen_ai.client.inference.time_per_output_chunk` → `gen_ai.client.operation.time_per_output_chunk` | No | New field: metric name in `add`:<pre>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: gen_ai.client.operation.time_to_first_chunk<br>    upsert: true</pre> | ✅ | Medium |
| Error attribute derived for some values | MCP `error.type` from `rpc.response.status_code` for client-side JSON-RPC errors (`-32700`, `-32600`, `-32601`, `-32602`, `-32002`) | Partially: one policy per code:<pre>matches:<br>  - target: {datapoint_attribute: [rpc.response.status_code]}<br>    predicate: {equals: "-32700"}<br>add:<br>  - target: {datapoint_attribute: [error.type]}<br>    value: "-32700"</pre> | New field: `from` in `add`:<pre>matches:<br>  - target: {datapoint_attribute: [rpc.response.status_code]}<br>    predicate: {regex: "^(-32700|-32600)$"}<br>add:<br>  - target: {datapoint_attribute: [error.type]}<br>    from: {datapoint_attribute: [rpc.response.status_code]}</pre> | ✅ | Medium |
| Span status change | MCP server spans for client-side errors | No | New: span transforms, `add` on `record_field`:<pre>add:<br>  - target: {record_field: SPAN_RECORD_FIELD_STATUS_CODE}<br>    value: ERROR<br>    upsert: true</pre> | ✅ | Medium |
| Attribute rename | `gen_ai.usage.cache_write.input_tokens` → `gen_ai.usage.cache_creation.input_tokens` (spans, events), `gen_ai.retrieval.top_k` → `gen_ai.request.top_k` | Yes: `rename`:<pre>rename:<br>  - from: {span_attribute: [gen_ai.usage.cache_write.input_tokens]}<br>    to: gen_ai.usage.cache_creation.input_tokens</pre> | - | ✅ | Low |
| Attribute type change | `gen_ai.request.top_k`, `gen_ai.retrieval.top_k` int → double | No | New policy function: `convert`:<pre>convert:<br>  - target: {span_attribute: [gen_ai.request.top_k]}<br>    type: double</pre> | ✅ | Low |
| Attributes missing on some spans | `gen_ai.agent.id`, `gen_ai.agent.version`, `gen_ai.provider.name`, cache token counts on internal `invoke_agent` spans | No | Impossible (cache counts would need aggregation over child spans) | ❌ | Low |
| JSON content edits | `gen_ai.output.messages[].finish_reason` from `gen_ai.response.finish_reasons` | No | - | ❌ | Low |

### K8s

#### Collector (1.18.0) → semconv 1.44.0

| Type of change | Examples | Existing policy functions | How to transform | Possible | Impact |
|---|---|---|---|---|---|
| Metric rename | `k8s.deployment.available` → `k8s.deployment.pod.available`, `k8s.container.restarts` → `k8s.container.restart.count`, `k8s.container.cpu_limit` → `k8s.container.cpu.limit.desired`, `k8s.node.allocatable_cpu` → `k8s.node.cpu.allocatable` | No | New field: metric name in `add`:<pre>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: k8s.container.restart.count<br>    upsert: true</pre> | ✅ | High |
| State encoded in value | `k8s.node.condition_ready` → `k8s.node.condition.status` with `k8s.node.condition.type` and `k8s.node.condition.status`, `k8s.pod.phase` (1-5) → `k8s.pod.status.phase` (0/1) with `k8s.pod.status.phase`, `k8s.pod.status_reason`, `k8s.namespace.phase` | No | New field: `datapoint_field: DATAPOINT_FIELD_VALUE`. The `0` series for the other states can't be created:<pre>rename:<br>  - from: {datapoint_field: DATAPOINT_FIELD_VALUE}<br>    to: k8s.pod.status.phase<br>add:<br>  - target: {datapoint_field: DATAPOINT_FIELD_VALUE}<br>    value: 1<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: k8s.pod.status.phase<br>    upsert: true<br>map:<br>  - target: {datapoint_attribute: [k8s.pod.status.phase]}<br>    values: {1: Pending, 2: Running}</pre> | ⚠️ | High |
| Metric rename, meaning change | `k8s.container.cpu_limit_utilization` → `k8s.container.cpu.limit.utilization`: spec limit vs current limit, which differ while a resize is pending | No | New field: metric name in `add`, approximate | ⚠️ | Medium |
| Attribute rename | `interface` → `network.interface.name`, `direction` → `network.io.direction` on network metrics | Yes: `rename`:<pre>rename:<br>  - from: {datapoint_attribute: [direction]}<br>    to: network.io.direction</pre> | - | ✅ | Medium |
| Resource attribute rename | `k8s.resource_quota.name` → `k8s.resourcequota.name`, `k8s.replication_controller.uid` → `k8s.replicationcontroller.uid`, `container.runtime` → `container.runtime.name` | Yes: `rename`:<pre>rename:<br>  - from: {resource_attribute: [container.runtime]}<br>    to: container.runtime.name</pre> | - | ✅ | Medium |
| Templated attribute keys | `k8s.pod.labels.<key>` → `k8s.pod.label.<key>`, same for annotations, Node and Namespace | No | New policy function: `rename_prefix`:<pre>rename_prefix:<br>  - from: {resource_attribute: [k8s.pod.labels.]}<br>    to: k8s.pod.label.</pre> | ✅ | Medium |
| Metric split by attribute value | `k8s.resource_quota.hard_limit` with `resource=limits.memory` → `k8s.resourcequota.memory.limit.hard`, same for `used` and OpenShift `openshift.clusterquota.*` | Partially: `equals` matcher, `remove` of the pinned `resource`. The metric name can't be written | New field: metric name in `add`:<pre>matches:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    predicate: {equals: k8s.resource_quota.hard_limit}<br>  - target: {datapoint_attribute: [resource]}<br>    predicate: {equals: limits.memory}<br>remove:<br>  - target: {datapoint_attribute: [resource]}<br>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: k8s.resourcequota.memory.limit.hard<br>    upsert: true</pre> | ✅ | Medium |
| Attribute moved from resource to data point | `k8s.volume.name`, `k8s.volume.type` on volume metrics | No | New field: `to_target` in `rename`:<pre>rename:<br>  - from: {resource_attribute: [k8s.volume.name]}<br>    to_target: {datapoint_attribute: [k8s.volume.name]}</pre> | ✅ | Medium |
| Attribute value parsed | Per-storage-class quota keys `<class>.storageclass.storage.k8s.io/requests.storage` → `k8s.storageclass.name` | No | New policy function: `split`:<pre>split:<br>  - from: {datapoint_attribute: [resource]}<br>    separator: .storageclass.storage.k8s.io/<br>    to: [k8s.storageclass.name, resource]</pre> | ✅ | Low |
| Unit change | ResourceQuota CPU millicores → cores | No | Query time only: multiply results by 0.001 | ⚠️ | Low |
| Metrics merged, value derived | `*.memory.major_page_faults` → `*.paging.faults` with `system.paging.fault.type=major`. `*.memory.page_faults` → `minor` needs total minus major | No | New field: metric name in `add` for `major`. `minor` is impossible | ⚠️ | Low |
| Instrument type change | Gauge → UpDownCounter for most metrics | No | Impossible. Prometheus exports both as gauges | ❌ | Low |

#### Semconv 1.44.0 → Collector (1.18.0)

| Type of change | Examples | Existing policy functions | How to transform | Possible | Impact |
|---|---|---|---|---|---|
| Metric rename | `k8s.deployment.pod.available` → `k8s.deployment.available`, `k8s.container.restart.count` → `k8s.container.restarts`, `k8s.container.cpu.limit.desired` → `k8s.container.cpu_limit`, `k8s.node.cpu.allocatable` → `k8s.node.allocatable_cpu` | No | New field: metric name in `add`:<pre>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: k8s.container.restarts<br>    upsert: true</pre> | ✅ | High |
| State encoded in attribute | `k8s.node.condition.status` with `k8s.node.condition.type` → `k8s.node.condition_ready` etc., `k8s.pod.status.phase` with `k8s.pod.status.phase` → `k8s.pod.phase` (1-5), `k8s.pod.status.reason` | No | New field: `datapoint_field: DATAPOINT_FIELD_VALUE`. Drop the `0` series with a filter policy. Node conditions need one policy per condition type:<pre>matches:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    predicate: {equals: k8s.pod.status.phase}<br>  - target: {datapoint_field: DATAPOINT_FIELD_VALUE}<br>    predicate: {equals: 1}<br>rename:<br>  - from: {datapoint_attribute: [k8s.pod.status.phase]}<br>    to_target: {datapoint_field: DATAPOINT_FIELD_VALUE}<br>    upsert: true<br>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: k8s.pod.phase<br>    upsert: true<br>map:<br>  - target: {datapoint_field: DATAPOINT_FIELD_VALUE}<br>    values: {Pending: 1, Running: 2}</pre> | ⚠️ | High |
| Metric rename, meaning change | `k8s.container.cpu.limit.utilization` → `k8s.container.cpu_limit_utilization`: current limit vs spec limit, which differ while a resize is pending | No | New field: metric name in `add`, approximate | ⚠️ | Medium |
| Attribute rename | `network.interface.name` → `interface`, `network.io.direction` → `direction` on network metrics | Yes: `rename`:<pre>rename:<br>  - from: {datapoint_attribute: [network.io.direction]}<br>    to: direction</pre> | - | ✅ | Medium |
| Resource attribute rename | `k8s.resourcequota.name` → `k8s.resource_quota.name`, `k8s.replicationcontroller.uid` → `k8s.replication_controller.uid`, `container.runtime.name` → `container.runtime` | Yes: `rename`:<pre>rename:<br>  - from: {resource_attribute: [container.runtime.name]}<br>    to: container.runtime</pre> | - | ✅ | Medium |
| Templated attribute keys | `k8s.pod.label.<key>` → `k8s.pod.labels.<key>`, same for annotations, Node and Namespace | No | New policy function: `rename_prefix`:<pre>rename_prefix:<br>  - from: {resource_attribute: [k8s.pod.label.]}<br>    to: k8s.pod.labels.</pre> | ✅ | Medium |
| Metrics merged by attribute value | `k8s.resourcequota.memory.limit.hard` → `k8s.resource_quota.hard_limit` with `resource=limits.memory`, same for `used` and OpenShift `openshift.clusterquota.*` | Partially: `add` on the attribute. The metric name can't be written | New field: metric name in `add`:<pre>matches:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    predicate: {equals: k8s.resourcequota.memory.limit.hard}<br>add:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    value: k8s.resource_quota.hard_limit<br>    upsert: true<br>  - target: {datapoint_attribute: [resource]}<br>    value: limits.memory</pre> | ✅ | Medium |
| Attribute moved from data point to resource | `k8s.volume.name`, `k8s.volume.type` on volume metrics | No | Impossible: one resource carries many volumes | ❌ | Medium |
| State encoded in attribute, single value | `k8s.namespace.phase` with `k8s.namespace.phase=active` → value-encoded `k8s.namespace.phase` | Partially: `remove` of the pinned phase on the `active` series, drop the other phases with a filter policy:<pre>matches:<br>  - target: {descriptor_field: METRIC_DESCRIPTOR_FIELD_NAME}<br>    predicate: {equals: k8s.namespace.phase}<br>  - target: {datapoint_attribute: [k8s.namespace.phase]}<br>    predicate: {equals: active}<br>remove:<br>  - target: {datapoint_attribute: [k8s.namespace.phase]}</pre> | Terminating namespaces may be missing instead of `0`, unknown (`-1`) can't be produced | ⚠️ | Low |
| Attribute value built from attributes | `k8s.storageclass.name` → `<class>.storageclass.storage.k8s.io/requests.storage` quota keys | No | Needs value templates, out of scope | ❌ | Low |
| Unit change | ResourceQuota CPU cores → millicores | No | Query time only: multiply results by 1000 | ⚠️ | Low |
| Metric split, values summed | `*.paging.faults` (all types) → `*.memory.page_faults`, the `major` series also → `*.memory.major_page_faults` | No | Query time only: sum over `system.paging.fault.type`. A record can't go to two metrics | ⚠️ | Low |
| Instrument type change | UpDownCounter → Gauge for most metrics | No | Impossible. Prometheus exports both as gauges | ❌ | Low |

