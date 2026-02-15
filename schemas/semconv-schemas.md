# Semantic Conventions Schemas

**Status**: [Alpha][DocumentStatus]

<!-- toc -->

- [Semantic Conventions Schemas](#semantic-conventions-schemas)
  - [Definition schema](#definition-schema)
  - [Resolved schema](#resolved-schema)
    - [Resolved schema properties](#resolved-schema-properties)
  - [Materialized resolved schema](#materialized-resolved-schema)
    - [Materialized schema properties](#materialized-schema-properties)
  - [Diff schema](#diff-schema)

<!-- tocstop -->

> [!WARNING]
> This document describes a new (future) version of the Semantic Conventions YAML model.
> This model is not yet feature-complete and is under active development.

This document describes schemas that govern the lifecycle of semantic conventions:

- the [definition schema](#definition-schema) for authoring,
- the [resolved schema](#resolved-schema) for distribution,
- the [materialized resolved schema](#materialized-resolved-schema) for validation and documentation/code generation,
- and the [diff schema](#diff-schema) for tracking changes between versions.

## Definition schema

The *definition* schema is used to write conventions. Conventions can be defined in multiple files. The
syntax is focused on avoiding duplication, maximizing consistency, and ease of authoring.
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

## Resolved schema

The *resolved* schema is a single file produced from a set of definition schemas. It contains a resolved
registry and refinements that are self-contained. It is optimized for
distribution and in-memory representation.

The *resolved* schema is produced by the `weaver registry resolve` command.
See the [resolved JSON schema](/schemas/semconv.resolved.v2.json) for the
full reference.

The resolved version of the metric above would look like this:

```yaml
file_format: "resolved/2.0.0"
schema_url: https://opentelemetry.io/schemas/semconv/1.42.0
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
  - 888   # this is the index of `server.address` in the attribute_catalog
  - 1042  # this is the index of `my.operation.name` in attribute_catalog
  ...
  metrics:
  - name: my.client.operation.duration
    instrument: histogram
    unit: s
    attributes:
      - base: 1042  # this is the index of `my.operation.name` in attribute_catalog
        requirement_level: required
      - base: 888  # this is the index of `server.address` in the attribute_catalog
        requirement_level: recommended
      ...
refinements:
  metrics:
  - name: my.client.operation.duration  # refinements include original signal definitions
    instrument: histogram
    unit: s
    attributes:
      - base: 1042
        requirement_level: required
        ...
```

Instead of references, we see indexes of attributes along with overridden properties.

### Resolved schema properties

- **file_format**: Version of the resolved schema, `resolved/2.0.0` in this iteration
- **schema_url**: The Schema URL where this registry is or will be published
- **attribute_catalog**: Attribute catalog containing all attribute definitions and refinements. May include duplicate keys. Attribute properties:
  - `key`: Attribute key
  - `type`: Attribute type. Can be primitive, array, template, enum with members or Any
  - `examples`: Example values (optional)
  - Common properties: `brief`, `stability`, and optionally `note`, `deprecated`, and `annotations`.
- **registry**: All signal and attribute definitions in the registry
  - **attributes**: List of indexes (into `attributes_catalog` array) corresponding to original attribute definitions. Does not include duplicates.
  - **attribute_groups**: Public attribute groups
    - `id`: Unique identifier
    - `attributes`: List of attribute indexes (into `attributes_catalog` array) belonging to this group
    - Common properties: `brief`, `stability`, and optionally `note`, `deprecated`, and `annotations`.
  - **metrics**: Metric signal definitions
    - `name`: Unique metric name
    - `instrument`: Instrument type (counter, gauge, histogram, updowncounter)
    - `unit`: Measurement unit
    - `attributes`: List of metric attribute references (optional)
      - `base`: Index (into `attributes_catalog` array)
      - `requirement_level`: Requirement level of this attribute
    - `entity_associations`: Associated entity types (optional)
    - Common properties: `brief`, `stability`, and optionally `note`, `deprecated`, and `annotations`.
  - **spans**: Span signal definitions
    - `type`: Unique span type identifier
    - `name`: Span name specification (object with `note` field describing how the span name should be created)
    - `kind`: Span kind (client, server, internal, producer, consumer)
    - `attributes`: List of span attribute references (optional)
      - `base`: Index (into `attributes_catalog` array)
      - `requirement_level`: Requirement level of this attribute
      - `sampling_relevant`: Whether this attribute should be provided at span start time (optional)
    - `entity_associations`: Associated entity types (optional)
    - Common properties: `brief`, `stability`, and optionally `note`, `deprecated`, and `annotations`.
  - **events**: Event signal definitions
    - `name`: Unique event name
    - `attributes`: List of event attribute references (optional)
      - `base`: Index (into `attributes_catalog` array)
      - `requirement_level`: Requirement level of this attribute
    - `entity_associations`: Associated entity types (optional)
    - Common properties: `brief`, `stability`, and optionally `note`, `deprecated`, and `annotations`.
  - **entities**: Entity (resource) signal definitions
    - `type`: Unique entity type
    - `identity`: List of identity attribute references
      - `base`: Index (into `attributes_catalog` array)
      - `requirement_level`: Requirement level of this attribute
    - `description`: List of descriptive attribute references (optional)
      - `base`: Index (into `attributes_catalog` array)
      - `requirement_level`: Requirement level of this attribute
    - Common properties: `brief`, `stability`, and optionally `note`, `deprecated`, and `annotations`.
- **refinements**: Signal refinements that extend base signals for specific implementations.
  Refinements also contain the original definitions of signals.
  - **spans**: Span refinements
    - `id`: Unique identifier for the refinement
    - All properties of the refined span (as defined in the `signals.spans` section)
  - **metrics**: Metric refinements
    - `id`: Unique identifier for the refinement
    - All properties of the refined metric (as defined in the `signals.metrics` section)
  - **events**: Event refinements
    - `id`: Unique identifier for the refinement
    - All properties of the refined event (as defined in the `signals.events` section)

## Materialized resolved schema

The *materialized resolved* schema, or *materialized* for short (and also known as the *forge* schema in the weaver codebase),
is based on the *resolved* schema. It expands attribute indexes into actual attribute definitions.
It is optimized for consumption when generating code or documentation, and for validating telemetry where
full definitions of every signal and attribute are necessary.

When running `weaver registry generate` or `weaver registry update-markdown`, the data passed
to JQ filters in weaver config follows the *materialized resolved* schema.

When running `weaver registry live-check` with custom Rego policies, the schema of the
attributes and signals matches the *materialized resolved* schema too.

See the [materialized JSON schema](/schemas/semconv.materialized.v2.json) for the
full reference.

The materialized resolved schema of the original metric would look like:

```yaml
file_format: materialized/2.0.0
schema_url: https://opentelemetry.io/schemas/semconv/1.42.0
registry:
  attributes:
  ...
  - key: my.operation.name
    type: string
    stability: stable
    brief: My service operation name as defined in this Open API spec
    examples: ["get_object", "create_collection"]
  ...
  metrics:
  - name: my.client.operation.duration
    instrument: histogram
    unit: s
    attributes:
      - key: my.operation.name  # attribute index is expanded into actual attribute definition
        type: string
        brief: My service operation name as defined in this Open API spec
        stability: stable
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
  - name: my.client.operation.duration  # refinements include original signal definitions
    instrument: histogram
    unit: s
    attributes:
      - key: my.operation.name  # fully expanded attribute
        type: string
        brief: My service operation name as defined in this Open API spec
        stability: stable
        examples: ["get_object", "create_collection"]
        requirement_level: required
      ...
```

### Materialized schema properties

- **file_format**: Version of the materialized schema, `materialized/2.0.0` in this iteration
- **schema_url**: The Schema URL where this registry is or will be published
- **registry**: Same as in the *resolved* schema, but attribute references are fully expanded
- **refinements**: Same as in the *resolved* schema, but attribute references are fully expanded

## Diff schema

The *diff* schema represents changes between two versions of a semantic convention registry.
Weaver can produce a diff between two semantic convention registry versions.
See the [diff JSON schema](/schemas/semconv.diff.v2.json)
for the full reference.

Diffs are produced with the `weaver registry diff` command. If templates are provided,
the data conforming to the diff schema is passed to the JQ filter in the weaver config file.
If templates are not provided, the output of the command follows this schema.

The diff schema contains a single top-level property:

- **file_format**: Version of the diff schema, `diff/2.0.0` in this iteration
- **head_schema_url**: Schema URL of the head (newer) registry
- **baseline_schema_url**: Schema URL of the baseline (older) registry
- **head_version**: Version of the head registry, if provided in the manifest (optional). E.g., `1.42.0`
- **baseline_version**: Version of the baseline registry, if provided in the manifest (optional). E.g., `1.40.0`
- **registry**: A `registry` object containing change arrays for each telemetry type:
  - `attribute_changes`: Array of changes to attributes
  - `attribute_group_changes`: Array of changes to attribute groups
  - `metric_changes`: Array of changes to metrics
  - `span_changes`: Array of changes to spans
  - `event_changes`: Array of changes to events
  - `entity_changes`: Array of changes to entities

Each change array contains objects representing different types of changes:

- **added**:
  - `type`: "added"
  - `name`: The name of the added object (attribute key, metric name, entity type, event name, span type, or attribute group id)
- **removed**:
  - `type`: "removed"
  - `name`: The name of the removed object
- **renamed**:
  - `type`: "renamed"
  - `old_name`: Original name in the baseline registry
  - `new_name`: New name in the head registry
  - `note`: Context about the rename
- **obsoleted**:
  - `type`: "obsoleted"
  - `name`: The name of the deprecated object
  - `note`: Deprecation details
- **uncategorized**:
  - `type`: "uncategorized"
  - `name`: The name of the affected object
  - `note`: Context about the change
- **updated** (placeholder for future use):
  - `type`: "updated"

[DocumentStatus]: https://opentelemetry.io/docs/specs/otel/document-status