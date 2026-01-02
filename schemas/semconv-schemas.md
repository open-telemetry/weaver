# Semantic Conventions Schemas

<!-- toc -->

- [Semantic Conventions Schemas](#semantic-conventions-schemas)
  - [Source schema](#source-schema)
  - [Resolved schema](#resolved-schema)
    - [Resolved schema overview](#resolved-schema-overview)
  - [Diff schema](#diff-schema)

<!-- tocstop -->

**Status**: [Alpha][DocumentStatus]

> [!WARNING]
> This document describes a new (future) version of the Semantic Conventions YAML model.
> This model is not yet feature-complete and is under active development.

This document describes three schemas that govern the lifecycle of semantic conventions:
the source schema for authoring, the resolved schema for distribution, and the diff schema
for tracking changes between versions.

Semantic conventions are authored in YAML following the [source schema](#source-schema),
packaged and then consumed following the [resolved schema](#resolved-schema).

The difference between two versions of semantic conventions is described using the [diff schema](#diff-schema).

## Source schema

The *source* schema is used to write conventions. Conventions can be defined in multiple files. The
syntax is focused on avoiding duplication, maximizing consistency, and ease of authoring.
See the [source schema syntax](/schemas/semconv.source-syntax.v2.md) for details and examples,
and the [source JSON schema](/schemas/semconv.source-schema.v2.json) for the full reference.

For example, the source schema might look like this:

```yaml
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

## Resolved schema

The *resolved* schema is a single file that's produced from a set of sources. It contains fully resolved
signal definitions and refinements that are self-sufficient.

The resolved version of the example above would look like this:

```yaml
metrics:
- name: my.client.operation.duration
  instrument: histogram
  unit: s
  attributes:
    - key: my.operation.name
      brief: My service operation name as defined in this Open API spec
      stability: stable
      requirement_level: required
      type:
        members:
        - id: get_my_order
          ...
        - id: create_my_order
          ...
    - key: server.address
      type: string
      brief: My server host name
      note: MUST match regional endpoint
      ...
      examples: ["foo-us-west-prod.my.com"]
      requirement_level: recommended
    - key: server.port
    ...

```

Instead of references, we see actual attributes along with their properties.

The following commands produce data that follows the *resolved* schema:

- `weaver registry resolve`
- `weaver registry generate` - the data passed to JQ filters follows the resolved schema

See the [resolved JSON schema](/schemas/semconv.resolved-schema.v2.json) for the
full reference.

### Resolved schema overview

All definitions share these common properties: `brief`, `stability`, and optionally `note`, `deprecated`, and `annotations`.

**Top-level collections:**

- **attributes**: Fully expanded attribute definitions
  - `key`: Unique identifier
  - `type`: Primitive, array, template, or enum with members
  - `examples`: Example values (optional)
  - common properties
- **attribute_groups**: Named collections of attributes
  - `id`: Unique identifier
  - `attributes`: Resolved attributes in this group
  - common properties
- **signals**: All telemetry signals. Each signal definition is also included in
  the `refinements` section as a base refinement that can be specialized.
  - **metrics**: Metric signal definitions
    - `name`: Metric name
    - `instrument`: Instrument type (counter, gauge, histogram, updowncounter)
    - `unit`: Measurement unit
    - `attributes`: List of resolved metric attributes with requirement levels
    - `entity_associations`: Associated entity types (optional)
    - common properties
  - **spans**: Span signal definitions
    - `type`: Unique span type identifier
    - `name`: Span name pattern
    - `kind`: Span kind (client, server, internal, producer, consumer)
    - `attributes`: List of resolved span attributes with requirement levels and sampling-relevant flag
    - `entity_associations`: Associated entity types (optional)
    - common properties
  - **events**: Event signal definitions
    - `name`: Event name
    - `attributes`: List of resolved event attributes with requirement levels
    - `entity_associations`: Associated entity types (optional)
    - common properties
  - **entities**: Entity (resource) signal definitions
    - `type`: Entity type identifier
    - `identity`: Attributes that identify the entity
    - `description`: Attributes that describe the entity
    - common properties
- **refinements**: Signal refinements that extend base signals for specific implementations.
  Refinements also contain the original definitions of the signals.
  - **spans**: Specialized span refinements
    - `id`: Unique identifier for the refinement
    - all properties of the refined span (as defined in the signals section)
  - **metrics**: Specialized metric refinements
    - `id`: Unique identifier for the refinement
    - all properties of the refined metric (as defined in the signals section)
  - **events**: Specialized event refinements
    - `id`: Unique identifier for the refinement
    - all properties of the refined event (as defined in the signals section)

## Diff schema

The *diff* schema represents changes between two versions of a semantic convention registry.
Weaver can produce a diff between two semantic convention registry versions.
See the [diff JSON schema](/schemas/semconv.diff.v2.json)
for the full reference.

Diffs are produced with the `weaver registry diff` command. If templates are provided,
the data conforming to the diff schema is passed to the JQ filter in the weaver config file.
If templates are not provided, the output of the command follows this schema.

The diff schema contains a single top-level property:

- **registry**: A `registry` object containing change arrays for each telemetry type:
  - `attribute_changes`: Array of changes to attributes
  - `attribute_group_changes`: Array of changes to attribute groups
  - `metric_changes`: Array of changes to metrics
  - `span_changes`: Array of changes to spans
  - `event_changes`: Array of changes to events
  - `entity_changes`: Array of changes to entities

Each change array contains objects representing different types of changes:

- **added**:
  - `name`: The name of the added object (attribute key, metric name, entity type, event name, span type, or attribute group id)
  - `type`: "added"
- **removed**:
  - `name`: The name of the removed object
  - `type`: "removed"
- **renamed**:
  - `old_name`: Original name in the baseline registry
  - `new_name`: New name in the head registry
  - `note`: Context about the rename
  - `type`: "renamed"
- **obsoleted**:
  - `name`: The name of the deprecated object
  - `note`: Deprecation details
  - `type`: "obsoleted"
- **uncategorized**:
  - `name`: The name of the affected object
  - `note`: Context about the change
  - `type`: "uncategorized"

[DocumentStatus]: https://opentelemetry.io/docs/specs/otel/document-status