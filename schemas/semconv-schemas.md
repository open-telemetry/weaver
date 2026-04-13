# Semantic Conventions Schemas

**Status**: [Alpha][DocumentStatus]

<!-- toc -->

- [Semantic Conventions Schemas](#semantic-conventions-schemas)
  - [Authoring schemas](#authoring-schemas)
    - [Definition manifest](#definition-manifest)
    - [Definition schema](#definition-schema)
  - [Publication schemas](#publication-schemas)
    - [Publication manifest](#publication-manifest)
    - [Resolved schema](#resolved-schema)
      - [Resolved schema properties](#resolved-schema-properties)
  - [Other schemas](#other-schemas)
    - [Materialized resolved schema](#materialized-resolved-schema)
      - [Materialized schema properties](#materialized-schema-properties)
    - [Diff schema](#diff-schema)
  - [Common types](#common-types)
    - [Requirement level](#requirement-level)
    - [Common signal and attribute properties](#common-signal-and-attribute-properties)

<!-- tocstop -->

> [!WARNING]
> This document describes the v2 Semantic Conventions YAML model,
> which is under active development.

This document describes schemas that govern the lifecycle of semantic conventions,
organized by how they are used:

- [**Authoring schemas**](#authoring-schemas) — used when writing and developing a registry
- [**Publication schemas**](#publication-schemas) — produced when packaging a registry for distribution
- [**Other schemas**](#other-schemas) — consumed by weaver commands internally, not distributed as files

## Authoring schemas

### Definition manifest

The *definition manifest* is a YAML file that describes a registry under development — its identity,
stability, and dependencies on other registries. It is the starting point for any registry project.
See the [definition manifest JSON schema](/schemas/definition-manifest.v2.json) for the full reference.

Properties:

- **schema_url**: Schema URL that uniquely identifies this registry and its version. Must follow the
  OTel schema URL format: `http[s]://server[:port]/path/<version>`.
- **stability**: Stability level of the registry (optional, defaults to `development`).
  One of: `development`, `alpha`, `beta`, `release_candidate`, `stable`.
- **description**: Markdown description of the registry (optional).
- **dependencies**: Registries this registry builds on (optional). Currently at most one dependency
  is supported. Each dependency has:
  - `schema_url`: Schema URL of the dependency registry (required)
  - `registry_path`: Path to the dependency's files (optional). When omitted,
    the dependency is resolved by its `schema_url` alone. Can be:
    - A local directory or archive (`.zip`, `.tar.gz`)
    - A remote archive URL
    - A remote file URL (e.g. a published registry manifest)
    - A Git repository URL
    - A GitHub release asset URL (automatically resolved via the GitHub API)

    For private repositories, set the `WEAVER_HTTP_AUTH_TOKEN` or `GITHUB_TOKEN`
    environment variable to authenticate HTTP downloads.

For example, a definition manifest for a registry that extends OTel semantic conventions:

```yaml
schema_url: https://acme.com/schemas/my-registry/1.0.0
stability: development
description: My custom registry.
dependencies:
- schema_url: https://opentelemetry.io/schemas/1.{future}.0
```

### Definition schema

The *definition* schema is used to write conventions. Conventions can be defined in multiple files. The
syntax is focused on avoiding duplication, maximizing consistency, and easing authoring.
See the [definition schema syntax](/schemas/semconv-syntax.v2.md) for details and examples,
and the [JSON schema](/schemas/semconv.schema.v2.json) for the full reference.

For example, the definition schema might look like this:

```yaml
file_format: "definition/2"
attributes:
- key: my.operation.name
  type: string
  stability: development
  brief: My service operation name as defined in this Open API spec
  examples: ["get_object", "create_collection"]
...
metrics:
- name: my.client.operation.duration
  stability: stable
  instrument: histogram
  unit: s
  attributes:
    - ref: my.operation.name
    - ref_group: my.operation.server.attributes
    - ref: error.type
```

<!-- TODO: add link to multi-registry docs -->

## Publication schemas

Produced by [`weaver registry package`](/docs/usage.md#weaver-registry-package).

### Publication manifest

The *publication manifest* is produced by `weaver registry package` alongside the resolved schema.
Together they form a self-contained, distributable registry artifact. The publication manifest is the
stricter counterpart of the definition manifest — it requires a resolved schema to be present and
records its location.
See the [publication manifest JSON schema](/schemas/publication-manifest.v2.json) for the full reference.

Properties:

- **file_format**: `"manifest/2.0.0"`
- **schema_url**: Schema URL that uniquely identifies this registry and its version.
- **resolved_schema_uri**: URI pointing to the resolved schema file included in the package.
- **stability**: Stability level of the registry (optional, defaults to `development`).
- **description**: Description of the registry (optional).
- **dependencies**: Same structure as in the [definition manifest](#definition-manifest).

For example, the publication manifest produced from the definition manifest above:

```yaml
file_format: "manifest/2.0.0"
schema_url: https://acme.com/schemas/my-registry/1.0.0
resolved_schema_uri: https://acme.com/schemas/my-registry/1.0.0/resolved.yaml
stability: development
description: My custom registry
dependencies:
- schema_url: https://opentelemetry.io/schemas/1.{future}.0
```

### Resolved schema

The *resolved* schema is a single file produced from a set of definition schemas. It contains all
registry signals and refinements in a single, self-contained file. It is optimized for
distribution and in-memory representation.

The *resolved* schema is produced by `weaver registry package` alongside the publication manifest.
See the [resolved JSON schema](/schemas/semconv.resolved.v2.json) for the full reference.

The resolved version of the metric above would look like this:

```yaml
# returned from https://acme.com/schemas/my-registry/1.0.0/resolved.yaml
file_format: "resolved/2.0.0"
schema_url: https://opentelemetry.io/schemas/semconv/1.{future}.0
attribute_catalog:
...
- key: my.operation.name
  type: string
  stability: development
  brief: My service operation name as defined in this Open API spec
  examples: ["get_object", "create_collection"]
...
registry:
  attributes:
  - 888   # index of `server.address` in attribute_catalog
  - 1042  # index of `my.operation.name` in attribute_catalog
  ...
  metrics:
  - name: my.client.operation.duration
    instrument: histogram
    unit: s
    attributes:
      - base: 1042  # index of `my.operation.name`
        requirement_level: required
      - base: 888   # index of `server.address`
        requirement_level: recommended
      ...
refinements:
  metrics:
  - name: my.client.operation.duration
    instrument: histogram
    unit: s
    attributes:
      - base: 1042
        requirement_level: required
        ...
```

Attribute references are indexes into the `attribute_catalog` array, paired with per-signal properties such as `requirement_level`.

#### Resolved schema properties

- **file_format**: `"resolved/2.0.0"`
- **schema_url**: The Schema URL where this registry is or will be published
- **attribute_catalog**: All attribute definitions. May include duplicate entries for the same key when
  refinements override attribute properties. Each entry has:
  - `key`: Attribute key
  - `type`: Attribute type — primitive, array, template, or enum with members
  - `examples`: Example values (optional)
  - [Common properties](#common-signal-and-attribute-properties)
  - Note: `requirement_level` is *not* a catalog property — it is defined per signal on each attribute reference.
- **registry**: All signal and attribute definitions
  - **attributes**: Indexes into `attribute_catalog` for original attribute definitions. No duplicates.
  - **attribute_groups**: Public attribute groups
    - `id`: Unique identifier
    - `attributes`: Attribute indexes (into `attribute_catalog`)
    - [Common properties](#common-signal-and-attribute-properties)
  - **metrics**: Metric signal definitions
    - `name`: Unique metric name
    - `instrument`: Instrument type (`counter`, `gauge`, `histogram`, `updowncounter`)
    - `unit`: Measurement unit
    - `attributes`: Attribute references (optional)
      - `base`: Index into `attribute_catalog`
      - `requirement_level`: See [Requirement level](#requirement-level)
    - `entity_associations`: Associated entity types (optional)
    - [Common properties](#common-signal-and-attribute-properties)
  - **spans**: Span signal definitions
    - `type`: Unique span type identifier
    - `name`: Object with a `note` field describing how the span name should be constructed
    - `kind`: Span kind (`client`, `server`, `internal`, `producer`, `consumer`)
    - `attributes`: Attribute references (optional)
      - `base`: Index into `attribute_catalog`
      - `requirement_level`: See [Requirement level](#requirement-level)
      - `sampling_relevant`: Whether this attribute must be available at span start (optional)
    - `entity_associations`: Associated entity types (optional)
    - [Common properties](#common-signal-and-attribute-properties)
  - **events**: Event signal definitions
    - `name`: Unique event name
    - `attributes`: Attribute references (optional)
      - `base`: Index into `attribute_catalog`
      - `requirement_level`: See [Requirement level](#requirement-level)
    - `entity_associations`: Associated entity types (optional)
    - [Common properties](#common-signal-and-attribute-properties)
  - **entities**: Entity (resource) signal definitions
    - `type`: Unique entity type
    - `identity`: Attribute references for the attributes that uniquely identify an entity instance (required)
      - `base`: Index into `attribute_catalog`
      - `requirement_level`: See [Requirement level](#requirement-level)
    - `description`: Attribute references for non-identifying descriptive attributes (optional)
      - `base`: Index into `attribute_catalog`
      - `requirement_level`: See [Requirement level](#requirement-level)
    - [Common properties](#common-signal-and-attribute-properties)
- **refinements**: Signal refinements that extend base signals for specific implementations.
  Each refinement is a fully-resolved variant of a base signal.
  - **spans**: Span refinements — `id` plus all span properties
  - **metrics**: Metric refinements — `id` plus all metric properties
  - **events**: Event refinements — `id` plus all event properties

## Other schemas

These schemas are not distributed as files — they are constructed in memory by weaver commands and
passed to jq filters, templates, and Rego policies.

### Materialized resolved schema

The *materialized resolved* schema, or *materialized* for short, is based on the *resolved* schema.
It expands all attribute indexes into full attribute definitions, making it self-contained for code
generation, documentation, and telemetry validation.

> [!NOTE]
> The *forge* schema in the weaver codebase is an alias for the materialized schema.

See the [materialized JSON schema](/schemas/semconv.materialized.v2.json) for the full reference.

[`weaver registry generate`](/docs/usage.md#weaver-registry-generate),
[`weaver registry update-markdown`](/docs/usage.md#weaver-registry-update-markdown), and
[`weaver registry live-check`](/docs/usage.md#weaver-registry-live-check)
pass data conforming to this schema to jq filters, templates, and Rego policies.

The materialized version of the same metric would look like:

```yaml
schema_url: https://opentelemetry.io/schemas/semconv/1.{future}.0
registry:
  attributes:
  ...
  - key: my.operation.name
    type: string
    stability: development
    brief: My service operation name as defined in this Open API spec
    examples: ["get_object", "create_collection"]
  ...
  metrics:
  - name: my.client.operation.duration
    instrument: histogram
    unit: s
    attributes:
      - key: my.operation.name  # index replaced by full attribute definition
        type: string
        brief: My service operation name as defined in this Open API spec
        stability: development
        examples: ["get_object", "create_collection"]
        requirement_level: required
      - key: server.address
        type: string
        brief: Server domain name or IP address
        examples: ["foo-us-west-prod.my.com"]
        requirement_level: recommended
  ...
refinements:
  metrics:
  - name: my.client.operation.duration
    instrument: histogram
    unit: s
    attributes:
      - key: my.operation.name  # fully expanded
        type: string
        brief: My service operation name as defined in this Open API spec
        stability: development
        examples: ["get_object", "create_collection"]
        requirement_level: required
      ...
```

#### Materialized schema properties

- **schema_url**: The Schema URL where this registry is or will be published
- **registry**: Same structure as in the *resolved* schema, but all attribute references are replaced
  by complete attribute definitions. This applies to all signal types (metrics, spans, events, entities)
  and to `attribute_groups`.
- **refinements**: Same structure as in the *resolved* schema, but attribute references are fully expanded.

### Diff schema

The *diff* schema represents changes between two versions of a semantic convention registry.
It is produced by [`weaver registry diff`](/docs/usage.md#weaver-registry-diff). If templates are
provided, the data is passed to the jq filter in the weaver config file; otherwise the command
output follows this schema directly.

See the [diff JSON schema](/schemas/semconv.diff.v2.json) for the full reference.

Top-level properties:

- **head_schema_url**: Schema URL of the head (newer) registry
- **baseline_schema_url**: Schema URL of the baseline (older) registry
- **registry**: Change arrays for each telemetry type:
  `attribute_changes`, `attribute_group_changes`, `metric_changes`, `span_changes`, `event_changes`, `entity_changes`

Each change array contains objects with a `type` discriminator and type-specific fields:

| `type` | Additional fields | Description |
| --- | --- | --- |
| `"added"` | `name` | Object added in the head registry |
| `"removed"` | `name` | Object removed from the head registry |
| `"renamed"` | `old_name`, `new_name`, `note` | Object renamed between versions |
| `"obsoleted"` | `name`, `note` | Object discontinued without a replacement |
| `"uncategorized"` | `name`, `note` | Change that doesn't fit other categories |
| `"updated"` | *(none)* | Placeholder for field-level updates (future use) |

`name` is the identifier of the affected object — attribute key, metric name, entity type, event name, span type, or attribute group id.

## Common types

### Requirement level

`requirement_level` appears on every attribute reference in the resolved and materialized schemas.
It is either a simple string or an object carrying an explanation:

| Value | Meaning |
| --- | --- |
| `"required"` | Always required |
| `"recommended"` | Recommended by default |
| `"opt_in"` | Not collected by default |
| `{"conditionally_required": "<condition>"}` | Required when the condition holds |
| `{"recommended": "<reason>"}` | Recommended, with explicit rationale |
| `{"opt_in": "<reason>"}` | Opt-in, with explicit rationale |

### Common signal and attribute properties

All signals (metrics, spans, events, entities, attribute groups) and catalog attributes share these properties:

| Property | Required | Description |
| --- | --- | --- |
| `brief` | yes | Short description |
| `stability` | yes | Stability level |
| `note` | no | Extended description |
| `deprecated` | no | Deprecation notice and migration guidance |
| `annotations` | no | Arbitrary key-value metadata |

[DocumentStatus]: https://opentelemetry.io/docs/specs/otel/document-status
