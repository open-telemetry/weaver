# Semantic convention transformations

<!-- disable markdownlint requirement for tables to be aligned -->
<!-- markdownlint-disable-file MD060 -->

> [!WARNING]
> Experimental. The file format below is a draft ("transform/2.0"). Nothing
> consumes these files yet, apart from a query-rewrite prototype that was used
> to validate them.

This folder holds machine-readable versions of the
[migration guides](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/) and of the
[gRPC compatibility guide](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/compatibility/grpc.md).
Each file is a **transformation set**: a list of
[OTEP #4738 Telemetry Policy](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md)
policies plus a header that ties them to a source and a target schema version.

A transformation set can be applied in two ways. The policies are the same in both:

- **Data mode.** Convert telemetry records (OTLP metrics, spans, logs, resources),
  for example in a Collector processor.
- **Schema mode.** Apply the policies to the definitions in a resolved schema
  instead. Query rewriters, dashboard and alert migration tools, and validators
  use this mode. It never touches stored data: queries written against the
  target version are rewritten to also read data in the source shape.

Both modes MUST agree: converting the data and then running a query against the
target gives the same result as running the rewritten query on the original data.

- [Files](#files)
- [Syntax](#syntax)
  - [File structure](#file-structure)
  - [Matchers](#matchers)
  - [Keep](#keep)
  - [Transform](#transform)
  - [Metric views](#metric-views)
  - [Entities](#entities)
  - [Evaluation](#evaluation)
  - [Safety](#safety)
  - [What can't be expressed](#what-cant-be-expressed)
- [Conventions used in these files](#conventions-used-in-these-files)
- [What isn't covered](#what-isnt-covered)

## Files

Each guide has one file per direction. The reverse direction is written on its own, not inverted:
nothing can undo a `remove`, and a change that can be bridged one way often can't be bridged the other way.

| Area | File | Source → target | Guide |
|---|---|---|---|
| Code attributes | [`code/1.29.0_to_1.33.0.yaml`](code/1.29.0_to_1.33.0.yaml) | 1.29.0 → 1.33.0 | [code-attrs-migration](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/code-attrs-migration.md) |
| | [`code/1.33.0_to_1.29.0.yaml`](code/1.33.0_to_1.29.0.yaml) | 1.33.0 → 1.29.0 | |
| Database | [`db/1.24.0_to_1.33.0.yaml`](db/1.24.0_to_1.33.0.yaml) | 1.24.0 → 1.33.0 | [db-migration](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/db-migration.md) |
| | [`db/1.33.0_to_1.24.0.yaml`](db/1.33.0_to_1.24.0.yaml) | 1.33.0 → 1.24.0 | |
| HTTP | [`http/1.20.0_to_1.23.1.yaml`](http/1.20.0_to_1.23.1.yaml) | 1.20.0 → 1.23.1 | [http-migration](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/http-migration.md) |
| | [`http/1.23.1_to_1.20.0.yaml`](http/1.23.1_to_1.20.0.yaml) | 1.23.1 → 1.20.0 | |
| Kubernetes | [`k8s/collector-1.18.0_to_1.44.0.yaml`](k8s/collector-1.18.0_to_1.44.0.yaml) | Collector (1.18.0) → 1.44.0 | [k8s-migration](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/k8s-migration.md) |
| | [`k8s/1.44.0_to_collector-1.18.0.yaml`](k8s/1.44.0_to_collector-1.18.0.yaml) | 1.44.0 → Collector (1.18.0) | |
| RPC | [`rpc/1.37.0_to_1.45.0.yaml`](rpc/1.37.0_to_1.45.0.yaml) | 1.37.0 → "1.45.0" | [rpc-migration](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/rpc-migration.md) |
| | [`rpc/1.45.0_to_1.37.0.yaml`](rpc/1.45.0_to_1.37.0.yaml) | "1.45.0" → 1.37.0 | |
| gRPC (native) | [`grpc/grpc-42.42.0_to_1.45.0.yaml`](grpc/grpc-42.42.0_to_1.45.0.yaml) | gRPC A66/A72 → "1.45.0" | [compatibility/grpc](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/non-normative/compatibility/grpc.md) |
| | [`grpc/1.45.0_to_grpc-42.42.0.yaml`](grpc/1.45.0_to_grpc-42.42.0.yaml) | "1.45.0" → gRPC A66/A72 | |
| GenAI | [`gen-ai/1.41.0_to_gen-ai-dev-1.42.0-dev.yaml`](gen-ai/1.41.0_to_gen-ai-dev-1.42.0-dev.yaml) | 1.41.0 → gen-ai-dev 1.42.0-dev | [GenAI migration](https://github.com/lmolkova/semantic-conventions-genai/blob/1780533536101fee409056c8ff24ed99ab861f0a/docs/gen-ai/non-normative/migration.md) |
| | [`gen-ai/gen-ai-dev-1.42.0-dev_to_1.41.0.yaml`](gen-ai/gen-ai-dev-1.42.0-dev_to_1.41.0.yaml) | gen-ai-dev 1.42.0-dev → 1.41.0 | |

## Syntax

The rule syntax and evaluation model come from
[OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md),
with the operator and field names of
[opentelemetry-proto#865](https://github.com/open-telemetry/opentelemetry-proto/pull/865).
This section summarizes them and lists what transformation sets add.
MUST, MUST NOT, SHOULD and MAY follow RFC 2119.

### File structure

```yaml
file_format: transform/2.0
source:
  schema_url: https://opentelemetry.io/schemas/1.20.0   # required
  registry_path: https://github.com/open-telemetry/semantic-conventions@v1.20.0[model]   # optional
target:
  schema_url: https://opentelemetry.io/schemas/1.23.1   # required, must differ from source

policies:
  - id: http-server-duration                 # required, unique in the set
    name: http.server.duration becomes http.server.request.duration   # required
    description: Optional. Why, and caveats.
    metric:                                  # exactly one target
      matches:
        - {metric_field: name, equals: http.server.duration}
        - {datapoint_attribute: [http.method], exists: true}
      transform:
        rename: [{datapoint_attribute: [http.method], to: http.request.method}]
        add: [{metric_field: name, value: http.server.request.duration, upsert: true}]

  - id: http-server-duration-view
    name: http.server.duration ms -> s
    metric_view:                             # aggregation changes live in a view
      matches:
        - {metric_field: name, equals: http.server.duration}
        - {datapoint_attribute: [http.method], exists: true}
      view:
        scale: [{metric_field: unit, from: ms, to: s}]
```

The header is a thin wrapper. It carries what a policy set can't: the source and
target versions. The wrapper never changes what a policy means: `policies` on its
own is a valid policy set that SDKs and the Collector can run.

`source` and `target` have the same shape as a dependency in a Weaver manifest:

| Field | Required | Meaning |
|---|---|---|
| `schema_url` | yes | Identifies the schema and its version. It must follow the OTel schema URL format and include a version, but doesn't have to be fetchable. |
| `registry_path` | no | Where the schema's files are, as a Weaver [virtual directory](https://github.com/open-telemetry/weaver/blob/main/docs/vdir.md) string (local directory or archive, remote archive or manifest, Git repository). When omitted, the schema is resolved from `schema_url`. |

`.weaver.toml` `[resolve.schema_url_overrides]` can point any schema URL to a local
checkout without editing the file. So a transformation from telemetry that isn't
defined with Weaver yet (for example native gRPC) can describe that telemetry as a
local Weaver schema and point to it with `registry_path`; its schema URL doesn't
need to be published.

`policies` is a list of `Policy` objects: `id` and `name` (required),
`description`, `enabled` and `labels` (optional), and exactly one target:

| Target | Defined by | Applies to |
|---|---|---|
| `metric` | [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md) | Metric name and datapoint attributes. Only `rename` and `add`. |
| `metric_view` | transformation sets | Metric streams: dropping attributes (re-aggregation) and unit scaling. See [Metric views](#metric-views). |
| `trace` | [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md) | Spans |
| `log` | [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md) | Log records and events. Events match on `log_field: event_name`. |
| `entity` | transformation sets | Resources (resource attributes) |

A change that affects several signals needs one policy per signal. For example,
an attribute renamed on spans and on metrics needs a `trace` policy and a
`metric` policy. Signal targets only touch the signal's own name, fields and
attributes; resource attributes change only through `entity` policies.

One set covers the whole migration from source to target, with no chaining. If a
migration spans several releases, the set holds the changes of all of them,
including names that only existed in an intermediate release. A set MAY be split
across several files with identical headers; the policy lists are concatenated
and duplicate ids are an error. Parsers MUST reject unknown top-level keys.

### Matchers

`matches` is a non-empty list, and all entries must hold. Each entry names one
field and one operator:

```yaml
matches:
  - {metric_field: name, equals: gen_ai.client.token.usage}
  - {datapoint_attribute: [gen_ai.token.type], equals: input}
  - {span_kind: client}
  - {trace_field: name, glob: "Sent.*"}
  - {span_attribute: [db.system], in: [postgresql, oracle], negate: true}
  - {span_attribute: [rpc.grpc.status_code], equals: 0}
  - {log_field: event_name, equals: gen_ai.client.inference.operation.details}
```

| Operator | Meaning | Defined by |
|---|---|---|
| `equals` | The value equals a scalar. Typed: the YAML scalar type is the value type, so `equals: 0` matches the int `0` and `equals: "0"` matches the string `"0"`. | #865 |
| `in` | The value equals any entry of a list. A string entry may be a glob. | **proposed**, not in #865 |
| `glob` | The whole string matches the pattern: `*` is any run of characters (including none), `?` is exactly one character. There's no escape and no character class. Same wildcard syntax as [Declarative Configuration](https://github.com/open-telemetry/opentelemetry-configuration/blob/main/schema-guidelines.md). | **proposed**, not in #865 |
| `exists` | `true`: the field is present. `false`: it's absent. | #865 |
| `contains` | An array element, or a substring of a string. | #865 |
| `gt`, `gte`, `lt`, `lte` | Numeric comparison. Not used in these files. | #865 |

#865 also defines `regex` (RE2). These files don't use it: `in` and `glob` cover
every case, and they're the proposed replacement for `regex` in #865, because
RE2 is hard to support in every SDK and hard to analyze statically.

- `negate: true` inverts any operator. `negate` on an absent attribute matches. For example,
  `{span_attribute: [gen_ai.operation.name], equals: retrieval, negate: true}`
  holds for spans without `gen_ai.operation.name`.
- There are no regular expressions and no case-insensitive matching. Every operator
  translates directly into PromQL label matchers and SQL predicates, and inverts exactly.
  A case-insensitive match is written as an `in` list of the spellings that occur, for
  example `in: [ok, OK]`.
- `{span_kind: client}` is shorthand for a span kind `equals` matcher.

| Target | Fields |
|---|---|
| `metric`, `metric_view` | `metric_field` (`name`, `unit`, ...), `metric_type`, `datapoint_attribute` |
| `trace` | `trace_field` (`name`, ...), `span_kind`, `span_status`, `span_attribute`, `event_name`, `event_attribute` |
| `log` | `log_field` (including `event_name`), `log_attribute` |
| `entity` | `entity_field` (`type`), `resource_attribute` |

Span type isn't on OTLP, so `trace` policies can't match on it.

### Keep

The keep stage is [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md)'s,
restricted to dropping everything: `keep: false` (or `keep: none` on logs). It
means "this data has no equivalent in the target". Sampling values MUST NOT
appear: a migration either converts data or doesn't. A dropping policy MUST NOT
have a `transform`.

Only one policy in these files drops data: `rev-k8s-namespace-phase-drop-other`,
which drops the stable per-phase `k8s.namespace.phase` series that the old
value-encoded metric can't represent. Other data without an equivalent is left
as-is, because its names don't collide with anything in the target.

### Transform

The transform shape is [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md)'s `LogTransform`:
one list per operation, each entry naming one field. Transformation sets use the
same shape for every target, and add `set_role` for entities.

```yaml
transform:
  remove: [{span_attribute: [net.sock.family]}]                     # not on metrics
  redact: [{span_attribute: [db.statement], replacement: "[REDACTED]"}]
  rename: [{span_attribute: [http.method], to: http.request.method, upsert: false},
           {span_attribute: [db.redis.database_index], to: db.namespace, convert: string}]
  add: [{span_attribute: [rpc.grpc.status_code], value: 0},
        {metric_field: name, value: http.server.request.duration, upsert: true}]
  set_role: [{resource_attribute: [k8s.pod.uid], role: identifying}] # entity only
```

Operations run in a fixed order, whatever order they're written in.
Every field reference uses the **source** name, except `to` and the field set by `add`.

| Order | Op | Defined by | Meaning |
|---|---|---|---|
| 1 | `remove` | [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md) | Removes the attribute. Not allowed on `metric`: dropping a metric attribute merges series, so it's a [view](#metric-views) operation. |
| 2 | `redact` | [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md) | Replaces the value with `replacement` (default `[REDACTED]`). |
| 3 | `rename` | [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md) | Moves an attribute's value to the key `to`, atomically. If `to` already has a value, `upsert: true` overwrites it and `upsert: false` (default) keeps it. Either way the old key goes away. Only for attribute keys: signal names are set with `add`. |
| 4 | `add` | [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md) | Sets an attribute, or a scalar field such as `metric_field: name` or `log_field: event_name`, to `value`, honoring `upsert` (default `false`). |
| 5 | `set_role` | transformation sets | Entities only. Moves a resource attribute between the entity's identifying and descriptive attributes. |

- **Type conversion is a `rename` option.** `convert: int | double | bool | string`
  changes the value's type as part of the move. It fails open: a value that
  doesn't convert isn't moved and keeps its key and type. An in-place type change
  is a `rename` whose `to` equals the source key:
  `{span_attribute: [gen_ai.request.top_k], to: gen_ai.request.top_k, convert: int}`.
- **`add` values are typed.** The YAML scalar type is the attribute type:
  `value: 0` writes an int, `value: "2"` writes a string. Values follow the
  attribute's type in the target conventions.
- **Signal names are scalar fields.** A metric or event changes its name with
  `add: [{metric_field: name, value: <new name>, upsert: true}]`, not with `rename`.
  The policy's matchers still see the input name (see [Evaluation](#evaluation)).

### Metric views

`metric_view` policies hold the operations that change aggregation or values.
They map to SDK Views and Collector aggregation, not to record transforms:

```yaml
- id: rpc-server-duration-view
  name: rpc.server.duration ms -> s, rpc.service and network.* dropped
  metric_view:
    matches: [{metric_field: name, equals: rpc.server.duration}]   # same matchers as metric
    view:
      drop_attributes:
        - {datapoint_attribute: [rpc.service]}
        - {datapoint_attribute: [network.transport]}
        - {datapoint_attribute: [network.type]}
      scale: [{metric_field: unit, from: ms, to: s}]
```

| Op | Meaning |
|---|---|
| `drop_attributes` | Re-aggregates the stream without the listed attributes. Points that end up with identical attributes are aggregated (exact for sums and histograms). Sets SHOULD NOT drop gauge attributes. |
| `scale` | Changes the unit from `from` to `to` (UCUM, same dimension). Multiplies values, sums, min/max and explicit bucket bounds by the factor. |

A view applies to the stream after the record transforms of the same set, so
`drop_attributes` names the attribute as it is after the transforms. In these
files a view never drops a key that a transform renames. A view policy's id ends
in `-view`; when it splits off from a `metric` policy, it repeats that policy's matchers.

Keeping views separate means record transforms are always rewritable query by
query, and only view policies need a per-query decision (for example, PromQL can't
rescale histogram buckets).

### Entities

```yaml
- id: k8s-replicationcontroller-name
  name: k8s.replication_controller.name becomes k8s.replicationcontroller.name
  entity:
    matches: [{resource_attribute: [k8s.replication_controller.name], exists: true}]
    transform:
      rename: [{resource_attribute: [k8s.replication_controller.name], to: k8s.replicationcontroller.name}]
```

`entity_field: type` reads the type from the resource's entity refs. A policy that
only matches on resource attributes applies to every resource. A renamed key keeps
its role (identifying or descriptive); an added key is descriptive unless `set_role` says otherwise.

### Evaluation

Policies are independent, as in [OTEP #4738](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/4738-telemetry-policy.md).
No policy runs before another, and no policy sees another policy's output. For each record:

1. Find every enabled policy whose matchers hold on the **input** record.
2. If any of them drops the record, it's removed.
3. Otherwise, collect the transform operations of all matching policies and run them by operation type, in the order above.
4. For metrics, apply the matching `metric_view` policies to the result.

Commutativity, idempotency and determinism are MUSTs. Where OTEP #4738 settles two
policies writing the same field by policy-id order, transformation sets forbid
the case instead: two policies that can match the same record MUST NOT apply the
same operation to the same field with different parameters. Different operations
on the same field are fine, because the fixed order settles them. For example,
a `remove` and a `rename` of the same span attribute: `remove` runs first, so the rename finds nothing.

This makes two patterns safe, and the files use both:

```yaml
# Value map, emulated for one value: the general policy renames the key,
# the value policy overwrites the value (add runs after rename).
- id: rpc-span-system-rename
  name: rpc.system renamed to rpc.system.name
  trace:
    matches: [{span_attribute: [rpc.system], exists: true}]
    transform:
      rename: [{span_attribute: [rpc.system], to: rpc.system.name}]
- id: rpc-span-system-value-dubbo
  name: rpc.system apache_dubbo becomes rpc.system.name dubbo
  trace:
    matches: [{span_attribute: [rpc.system], equals: apache_dubbo}]
    transform:
      add: [{span_attribute: [rpc.system.name], value: dubbo, upsert: true}]
```

On metrics this is how a single attribute value is fixed: a rename plus an `add`
with `upsert: true`, never `remove` + `add`. If the value policy matches records
the general rename policy doesn't, it carries the same rename itself (identical
parameters, so the two don't conflict).

Data mode fails open, as in OTEP #4738: a policy that can't be parsed or applied
is skipped, and the telemetry passes through. Schema mode fails closed: a query
rewriter that can't translate a policy MUST reject the queries that touch the
policy's signals, naming the policy and the reason, rather than return wrong results.

### Safety

Every policy MUST leave data that's already in the target shape unchanged.

That's what makes a set safe to apply to all telemetry, old or new, with or
without a schema URL: target-shaped data passes through untouched. In practice a
policy matches on something only the source has: a source-only metric name or
attribute, or a value the target never uses.

Some changes would have to touch target-shaped data, so they can't be policies:

- **A meaning change under the same name**, for example `server.address` on RPC spans.
- **A name the target reuses for something else**, for example `v8js.memory.heap.limit` in 1.42.
- **A unit change under the same name**, for example `http.server.duration`, in seconds since 1.21.

### What can't be expressed

The files leave these changes out and list them in their header comments.

**Planned additive extensions.** These would close most of the gaps. Each one
keeps moves atomic and reads the pre-move state, so it fits the operation order above:

| Extension | Examples it unlocks |
|---|---|
| Value map on `rename` (`map`, with `unmapped: keep \| skip \| default`) | `db.system` values, gRPC status codes, HTTP methods → `_OTHER`, `rpc.system` values, protocol version `2.0` → `2`. Emulated today with one policy per value. |
| Copy (`keep_source` on `rename`) | `error.type` from a status code, the HTTP `client.address` fallback, MCP `error.type` |
| Split (one key into two, on a literal delimiter) | `http.target` → `url.path` + `url.query`, `rpc.method` → `rpc.service` + `rpc.method`, the `Sent.`/`Recv.` span-name prefixes |
| Concatenate (several keys into one) | `rpc.service` + `rpc.method`, `url.path` + `url.query` → `http.target`, `db.namespace` for SQL Server instances, `code.namespace` + `code.function` |
| Prefix rename (templated keys) | `http.request.header.<key>`, `rpc.request.header.<key>`, `k8s.pod.label.<key>`, `db.operation.parameter.<key>` |

**Not goals for policies.** These won't become policy operations:

- URI parsing (`grpc.target` → `server.address` + `server.port`);
- JSON content edits (GenAI `system_instructions` filtering, `finish_reason` back-fill);
- metric fan-out, histogram → counter and other instrument changes, cross-metric arithmetic
  (GenAI `gen_ai.client.token.usage` → usage counters);
- moves between signals (exception span events → log events);
- cross-span aggregation (GenAI cache token counts on internal agent spans);
- value ↔ attribute encodings (K8s phase and condition metrics), at least for now.

Span name rewrites and span status changes aren't covered by either list yet.

## Conventions used in these files

- **Schema URLs.**
  - `https://opentelemetry.io/schemas/1.45.0` is a placeholder for the first
    release that contains the stable RPC conventions (`rpc.status_code`,
    `rpc.request.header.*`). They're on `main` but not released yet.
  - `https://grpc.io/schemas/42.42.0` is invented. gRPC publishes conventions
    ([A66](https://github.com/grpc/proposal/blob/master/A66-otel-stats.md),
    [A72](https://github.com/grpc/proposal/blob/master/A72-open-telemetry-tracing.md)), not a schema URL.
  - The Kubernetes "old" side is what the Collector emits; it stamps
    `https://opentelemetry.io/schemas/1.18.0`.
  - `https://opentelemetry.io/schemas/gen-ai-dev/1.42.0-dev` is the manifest URL of
    the GenAI conventions at the pinned commit. No schema file is published for it.
- **Intermediate releases.** Where producers shipped names in an intermediate release
  (HTTP 1.21 `server.socket.*`, DB 1.26 `db.client.connections.*`, RPC 1.39–1.44
  `rpc.response.status_code`), the set also covers them, as long as that doesn't conflict
  with the target.
- **HTTP 1.21 durations** were already in seconds under the old names, so only
  series that carry 1.20-only attributes are scaled.
- **HTTP/2 protocol version.** The 1.23.1 → 1.20.0 set assumes 1.20 producers
  reported HTTP/2 as `"2.0"`, as in the 1.20 examples.
- **Dual emission.** A producer that emits both shapes is counted twice. Sets don't deduplicate.

## What isn't covered

**Impossible** means no rule could ever be correct (the data was never captured,
the mapping is ambiguous, or the meaning changed under the same name).
**New op** means the source has the information but today's operations can't express it.
**High** marks items that block a golden signal: duration, error rate or throughput.

| Area | Direction | Not covered |
|---|---|---|
| Code | both | Impossible: `code.function.name` from 1.30 producers (short name under the same key); splitting `code.function.name` back. New op: concatenating `code.namespace` + `code.function`. |
| DB | 1.24 → 1.33 | Impossible: **`db.client.operation.duration` (High, new metric)**; PostgreSQL schema / Oracle service parts of `db.namespace`; new attributes. New op: **`error.type` (High)**, `db.namespace` for SQL Server instances, span names, `db.query.summary`, Elasticsearch path parts. |
| DB | 1.33 → 1.24 | Impossible: removed attributes (no source data). New op: splitting qualified `db.namespace` values, span names. |
| HTTP | 1.20 → 1.23.1 | Impossible: header keys with `_`; values derived from `X-Forwarded-*`; defaulted ports. New op: **`error.type` (High)**, `http.target` split, `_OTHER` span names, `client.address` fallback, default `server.port`. |
| HTTP | 1.23.1 → 1.20 | Impossible: original method on metrics (`_OTHER`); `net.host.name` on the server metric (opt-in in stable); `net.sock.*` on the client metric. New op: `http.target` concat, header keys, span names, default ports. |
| K8s | both | Impossible: `.current` limits and requests; pod CPU utilization (denominator changed); rounded `cpu` quota; namespace phase "unknown". New op: phase and condition value ↔ attribute, labels and annotations, quota CPU millicores, hugepage and storage-class keys, minor page faults, volume attributes, gauge → up-down counter. |
| RPC | 1.37 → 1.45 | Impossible: `server.address` / `server.port` (new meaning); response header vs trailer. New op: **`rpc.service` + `rpc.method` (High)**, **`error.type` (High)**, metadata keys, exception events, JSON-RPC error message. |
| RPC | 1.45 → 1.37 | Impossible: `server.address` meaning; header/trailer key collisions; removed attributes. New op: **splitting `rpc.method` (High)**, metadata keys, integer gRPC status, exception events. |
| gRPC | gRPC → OTel | New op: `server.address`/`server.port` from `grpc.target` (part is impossible: `xds:`, `unix:` targets); span name without `Sent.`/`Recv.`; `rpc.method` and `rpc.status_code` on spans; span status. Impossible: `error.type` for failures without a status code. Attempt metrics and spans have no equivalent and are left as-is. |
| gRPC | OTel → gRPC | New op: `grpc.target` from `server.address`/`server.port`; `Sent.`/`Recv.` span names; span status and status description. Impossible: attempt metrics and spans (no source data). |
| GenAI | 1.41 → dev | Impossible: **`gen_ai.invoke_agent.duration`, `gen_ai.execute_tool.duration`, `gen_ai.invoke_workflow.duration` (High, new metrics)**; cache, reasoning and per-invocation call metrics; billed vs consumed tokens; new attributes. New op: **token usage counters from the `gen_ai.client.token.usage` histogram (High)**, text-only `gen_ai.system_instructions`, MCP span status, event severity. |
| GenAI | dev → 1.41 | Impossible: `gen_ai.provider.name`, `gen_ai.agent.id`, `gen_ai.agent.version` on internal agent spans; `finish_reasons` `error` padding. New op: `finish_reason` back-fill, cache tokens on internal agent spans, skill span names, MCP span status. |

**GenAI #521.** The GenAI migration guide describes #521: inference metrics move to
`gen_ai.client.inference.*`, and the span type becomes `gen_ai.client.inference`.
The model at the pinned commit doesn't have it yet, so the files contain no #521
policies: applied to the pinned target, they would move target-shaped data. Once
#521 lands, add these policies (and the same renames back in the reverse file):

```yaml
- id: genai-521-inference-duration
  name: Inference operations move to gen_ai.client.inference.duration
  description: >
    Inference calls with a custom gen_ai.operation.name can't be told apart
    from other operations, so they stay on gen_ai.client.operation.duration.
  metric:
    matches:
      - {metric_field: name, equals: gen_ai.client.operation.duration}
      - {datapoint_attribute: [gen_ai.operation.name], in: [chat, generate_content, text_completion]}
    transform:
      add: [{metric_field: name, value: gen_ai.client.inference.duration, upsert: true}]
- id: genai-521-ttfc
  name: time_to_first_chunk renamed
  metric:
    matches: [{metric_field: name, equals: gen_ai.client.operation.time_to_first_chunk}]
    transform:
      add: [{metric_field: name, value: gen_ai.client.inference.time_to_first_chunk, upsert: true}]
- id: genai-521-tpoc
  name: time_per_output_chunk renamed
  metric:
    matches: [{metric_field: name, equals: gen_ai.client.operation.time_per_output_chunk}]
    transform:
      add: [{metric_field: name, value: gen_ai.client.inference.time_per_output_chunk, upsert: true}]
```
