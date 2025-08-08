# Semantic Convention YAML Language v2

**Status**: [Alpha][DocumentStatus]

> [!WARNING]
> This document describes a new (future) version of the Semantic Conventions YAML model. This model is not yet feature-complete and is under active development.

<!-- toc -->

- [Semantic Convention YAML Language v2](#semantic-convention-yaml-language-v2)
  - [Syntax](#syntax)
    - [`attributes` definition](#attributes-definition)
      - [Attribute Types](#attribute-types)
        - [Enums](#enums)
        - [Template type](#template-type)
    - [Attribute reference](#attribute-reference)
    - [`spans` definition](#spans-definition)
      - [Span name](#span-name)
    - [`entities` definition](#entities-definition)
    - [`events` definition](#events-definition)
    - [`metrics` definition](#metrics-definition)
    - [`imports` definition](#imports-definition)
    - [Stability levels](#stability-levels)
    - [Deprecated structure](#deprecated-structure)
      - [Rename](#rename)
      - [Obsolete](#obsolete)
      - [Uncategorized](#uncategorized)
    - [Annotations](#annotations)
      - [Code Generation Annotations](#code-generation-annotations)

<!-- tocstop -->

A JSON schema description of the syntax is available as [semconv.schema.v2.json](./semconv.schema.v2.json),
you can use it in your IDE to autocomplete and validate YAML.
If you use VSCode, check out [YAML Language Support](https://marketplace.visualstudio.com/items?itemName=redhat.vscode-yaml) extension.

This file provides human-readable documentation, but JSON schema should be considered as the source of truth.

> [!NOTE]
> This documents talks about syntax, refer to the [How to write conventions](https://github.com/open-telemetry/semantic-conventions/tree/main/docs/how-to-write-conventions#defining-attributes) if you're looking for guidance on how to design conventions.

## Syntax

A semantic convention file starts with `version: "2"` and may contain one or more of the following groups:

- `attributes`: Contains definitions of semantic attributes which may be applicable to all OpenTelemetry signals.
- `entities`: Contains definitions of entities.
- `events`: Contains definitions of events.
- `metrics`: Contains definitions of metric instruments.
- `spans`: Contains definitions of spans.
- `imports`: Allows importing attributes or signal definitions from a different semantic convention registry (dependencies on registries are declared in `registry_manifest.yaml`).

### `attributes` definition

Attributes section contains a list of attribute definitions.

Attributes capture important details about different kinds of telemetry items. Attributes are fully qualified with the `key` and
their semantical meaning remains the same whenever they are used.

Here's a simplified example of `server.address` and `server.port` attribute definitions:

```yaml
version: "2"
attributes:
  - key: server.address
    stability: development
    type: string
    brief: The domain name or IP address of the server.
    examples: ['example.com']
  - key: server.port
    stability: development
    type: int
    brief: The port number of the server.
    examples: [8080]
```

Attributes can only be defined inside the `attributes` group. Attribute definitions consist of the following properties:

- `key` - Required. String that uniquely identifies the attribute.
- `type` - Required. Defines [attribute type](#attribute-types)
- `brief` - Required. string. A short description of what this attribute represents
- `note` - Optional. string. A more elaborate description of the attribute.
- `stability` - Required. Specifies the [stability](#stability-levels) of the attribute
- `deprecated` - Optional, when present marks the attribute as deprecated. See [deprecated](#deprecated-structure) for the details.
- `annotations` - Optional. Map of annotations. Annotations are key-value pairs that provide additional information about
  the attribute. See [annotations](#annotations) for details.
- `examples` - Optional. List of example values for the attribute.

#### Attribute Types

The following types are supported:

- `string`
- `int`
- `double`
- `boolean`
- `string[]`
- `int[]`
- `double[]`
- `boolean[]`
- `any` - represents complex types. It's not yet possible to provide expected type definitions in YAML, but authors are encouraged to do it with JSON schema or other means.

In addition to the proto-level attribute type definitions, semantic conventions allow defining attributes of the following types:

- [enums](#enums) - Represents an attribute with a relatively small set of possible values. The actual type or attribute value is limited to `string` and `int`.
- [template](#template-type) - Represents a set of attributes with a common key prefix. The actual type of the attribute value is limited to one of the proto-level types listed above.

##### Enums

Enums are semantic convention concepts and do not have analogs in the OpenTelemetry specification or OTLP. Enums are used to define a known set of attribute values. Semantic convention enums are open by definition. See [semantic conventions stability](https://github.com/open-telemetry/opentelemetry-specification/blob/v1.47.0/specification/versioning-and-stability.md#semantic-conventions-stability) for the details.

Here's an example of an enum attribute definition:

```yaml
  - key: http.request.method
    stability: stable
    type:
      members:
        - id: connect
          value: "CONNECT"
          brief: 'CONNECT method.'
          stability: stable
        - id: delete
          value: "DELETE"
          brief: 'DELETE method.'
          stability: stable
        - id: get
          value: "GET"
          brief: 'GET method.'
          stability: stable
        - id: post
          value: "POST"
          brief: 'POST method.'
          stability: stable
        - id: put
          value: "PUT"
          brief: 'PUT method.'
          stability: stable
        # ...           
    brief: 'HTTP request method.'
```

and another example of int enum attribute

```yaml
  - key: rpc.grpc.status_code
    type:
      members:
        - id: ok
          brief: OK
          stability: development
          value: 0
        - id: cancelled
          brief: CANCELLED
          stability: development
          value: 1
        - id: unknown
          brief: UNKNOWN
          stability: development
          value: 2
        #... 
    stability: development
    brief: "The [numeric status code](https://github.com/grpc/grpc/blob/v1.33.2/doc/statuscodes.md) of the gRPC request."
```

Enum members have the following properties:

- `id` - Required. Identifies enum member within this enum.
- `brief` - Optional. A short description of what this enum member represents.
- `note` - Optional. A more elaborate description of the member.
- `stability` - Required. Specifies the [stability](#stability-levels) of the enum member.
- `deprecated` - Optional. When present marks the member as deprecated. See [deprecated](#deprecated-structure) for the details.
- `annotations` - Optional. Annotations are key-value pairs that provide additional information about the attribute. See [annotations](#annotations) for details.

Enum attributes can only be of type `int` and `string`, the type is deduced from the value.

##### Template type

A template type represents a set of attributes with a common key prefix. The syntax for defining template type attributes is the following:

`type: template[<actual_attribute_type>]`

The `<actual_attribute_type>` is one of the primitives, array, or `any`, but not an enum, and specifies the type of the actual attribute to be recorded on telemetry item.

The following is an example for defining a template type attribute:

```yaml
attributes:
  key: http.request.header
  stability: stable
  type: template[string[]]
  brief: >
    HTTP request headers, `<key>` being the normalized HTTP Header name (lowercase), the value being the header values.
  note: |
    ...

    Examples:

    - A header `Content-Type: application/json` SHOULD be recorded as the `http.request.header.content-type`
      attribute with value `["application/json"]`.
    - A header `X-Forwarded-For: 1.2.3.4, 1.2.3.5` SHOULD be recorded as the `http.request.header.x-forwarded-for`
      attribute with value `["1.2.3.4", "1.2.3.5"]` or `["1.2.3.4, 1.2.3.5"]` depending on the HTTP library.

  examples: [["application/json"], ["1.2.3.4", "1.2.3.5"]]
```

In this example the definition will be resolved into a set attributes `http.request.header.<key>` where `<key>` will be replaced by the actual HTTP header name, and the value of the attributes is of type `string[]` that carries the HTTP header value.

### Attribute reference

When defining a specific signal such as span, metric, event, or entity, you also specify a list of attributes this signal should contain.
Attributes cannot be defined on the signals themselves.

So signal definitions contain references to attribute definitions and may refine original attribute definition - for example, to make original definition
more specific and provide details on how and when to capture it in the scope of that signal or domain.

Attributes are referenced by their key. Here's an example of how to reference attributes when defining spans:

```yaml
spans:
  - type: http.client
    # ...
    attributes:
      - ref: http.request.method
        requirement_level: required
        sampling_relevant: true
```

When referencing spans, you can refine the following properties for the scope of signal being defined:

- `brief`
- `note`
- `examples`
- `annotations`
- `stability` can be changed from stable to unstable, but not the other way around
- `deprecated` can be changed from not-deprecated to deprecated, but not the other way around

The following properties can be defined on the attribute reference only:

- `requirement_level` - Optional - see [Requirement Levels](https://github.com/open-telemetry/semantic-conventions/blob/v1.36.0/docs/general/attribute-requirement-level.md) for the details.
- `sampling_relevant` - Optional - available on spans only - a boolean flag indicating if the attribute is (especially) relevant for sampling and
  thus should be set at span start. It defaults to `false`.

### `spans` definition

Spans section contains a list of span definitions. A span definition consists of the following properties:

- `type` - Required. Uniquely identifies span type such as `http.client`
- `kind` - Required. The kind of span. Must be one of:
  - `client` - Outgoing request
  - `server` - Incoming request
  - `producer` - Enqueue operation
  - `consumer` - Dequeue operation
  - `internal` - Internal operation
- `brief` - Required. A short description of the operation this span represents
- `note` - Optional. A more elaborate description of the operation
- `stability` - Required. Specifies the [stability](#stability-levels) of the span definition
- `name` - Required. Specification of how the [span name](#span-name) should be formatted.
- `deprecated` - Optional. When present, marks the span as deprecated. See [deprecated](#deprecated-structure) for details
- `attributes` - Optional. List of [attribute references](#attribute-reference) applicable to this span.
- `entity_associations` - Optional. List of entity types that can be associated with this span type
- `annotations` - Optional. Map of annotations. Annotations are key-value pairs that provide additional information about the span. See [annotations](#annotations) for details

Example:

```yaml
spans:
  - type: http.client
    name:
      note: "{http.request.method}"
    kind: client
    brief: Represents the client-side of HTTP request
    stability: stable
    attributes:
      - ref: http.request.method
        requirement_level: required
        sampling_relevant: true
      - ref: url.full
        requirement_level: required
        sampling_relevant: true
      # ...        
    entity_associations:
      - ref: service.instance
```

#### Span name

The `name` field specifies how the span name should be formatted. It consists of a `note` field that describes in a free form how to format span name based on the attributes. OpenTelemetry semantic conventions use `{action} {target}` format where action and target match attributes on that span. For example, [HTTP server span names](https://github.com/open-telemetry/semantic-conventions/blob/v1.36.0/docs/http/http-spans.md#name) match `{http.request.method} {http.route}` pattern in general case.

The span name structure may be evolved in the future to formally define the naming pattern.

### `entities` definition

Entities section contains a list of entity definitions. An entity is a collection of attributes that describe an object that telemetry can be associated with, such as a service instance, K8s pod, or CI/CD pipeline.

An entity definition consists of the following properties:

- `type` - Required. Uniquely identifies the entity type.
- `brief` - Required. A short description of what this entity represents.
- `note` - Optional. A more elaborate description of the entity.
- `stability` - Required. Specifies the [stability](#stability-levels) of the entity definition.
- `identity` - Required. List of [attribute references](#attribute-reference) that form the identity of the entity. These attributes uniquely identify an instance of the entity.
- `description` - Optional. List of [attribute references](#attribute-reference) that provide additional descriptive information about the entity but are not part of its identity.
- `deprecated` - Optional. When present, marks the entity as deprecated. See [deprecated](#deprecated-structure) for details.
- `annotations` - Optional. Map of annotations. Annotations are key-value pairs that provide additional information about the entity. See [annotations](#annotations) for details.

Here's an example of entity definition

```yaml
entities:
  - type: service
    brief: A service instance.
    stability: stable
    identity:
      - ref: service.name
        requirement_level: required
      - ref: service.namespace
      - ref: service.instance.id
    description:
      - ref: service.version
        role: descriptive
```

### `events` definition

Events section contains a list of event definitions. An event represents a discrete occurrence at a point in time, such as a request completion, system startup, or error condition.

An event definition consists of the following properties:

- `name` - Required. Uniquely identifies the event definition.
- `brief` - Required. A short description of what this event represents.
- `note` - Optional. A more elaborate description of the event.
- `stability` - Required. Specifies the [stability](#stability-levels) of the event definition.
- `attributes` - Optional. List of [attribute references](#attribute-reference) that can be set on this event type.
- `entity_associations` - Optional. List of entities that this event can be associated with.
- `deprecated` - Optional. When present, marks the event as deprecated. See [deprecated](#deprecated-structure) for details.
- `annotations` - Optional. Map of annotations. Annotations are key-value pairs that provide additional information about the event. See [annotations](#annotations) for details.

Here's an example of event definition:

```yaml
events:
  - name: exception
    brief: A software error was detected.
    stability: stable
    attributes:
      - ref: exception.type
        requirement_level: required
      - ref: exception.message
        requirement_level: required
      - ref: exception.stacktrace
        requirement_level: recommended
    entity_associations:
      - ref: service.instance
```

### `metrics` definition

Metrics section contains a list of metric definitions. A metric represents a measurement of a value over time, such as request duration, CPU usage, or error count.

A metric definition consists of the following properties:

- `name` - Required. Uniquely identifies the metric.
- `brief` - Required. A short description of what this metric represents.
- `note` - Optional. A more elaborate description of the metric.
- `unit` - Required. The unit in which the metric is measured matching [Unified Code for Units of Measure](https://unitsofmeasure.org/ucum.html).
- `instrument` - Required. The type of instrument used to record the metric. Must be one of:
  - `counter` - A value that can only go up or be reset to 0, used for counts
  - `updowncounter` - A value that can go up and down, used for sizes or amount of items in a queue
  - `gauge` - A value that can arbitrarily go up and down, used for temperature or current memory usage
  - `histogram` - Distribution of recorded values, used for latencies or request sizes
- `stability` - Required. Specifies the [stability](#stability-levels) of the metric definition.
- `attributes` - Optional. List of [attribute references](#attribute-reference) that can be set on this metric.
- `entity_associations` - Optional. List of entity types that this metric can be associated with.
- `deprecated` - Optional. When present, marks the metric as deprecated. See [deprecated](#deprecated-structure) for details.
- `annotations` - Optional. Map of annotations. Annotations are key-value pairs that provide additional information about the metric. See [annotations](#annotations) for details.

Here's an example of metric definition:

```yaml
metrics:
  - name: http.server.request.duration
    brief: Duration of HTTP server requests.
    unit: s
    instrument: histogram
    stability: stable
    attributes:
      - ref: http.request.method
        requirement_level: required
      - ref: http.response.status_code
        requirement_level: required
      # ...
    entity_associations:
      - ref: service.instance
```

### `imports` definition

Imports section allows referencing semantic conventions defined in other registries - for example when defining conventions within your company,
you may want to import OpenTelemetry semantic conventions.

An imports definition consists of optional lists of group name wildcards for different signal types:

- `entities` - Optional. List of entity type wildcards.
- `events` - Optional. List of event name wildcards.
- `metrics` - Optional. List of metric name wildcards.

Each wildcard can match one or more groups from the imported registry. For example:

```yaml
imports:
  entities:
    - k8s.*         # Import all Kubernetes entities
    - service       # Import service instance entity
  metrics:
    - http.server.*  # Import all HTTP server metrics
```

### Stability levels

The following stability levels are supported: `stable`, `development`, `alpha`, `beta`, `release_candidate`. See [OpenTelemetry stability definitions](https://github.com/open-telemetry/opentelemetry-specification/blob/v1.47.0/specification/document-status.md) for the details.

### Deprecated structure

The `deprecated` field indicates that a component (attribute, metric, event, etc.) should no longer be used. It supports several deprecation reasons:

#### Rename

Used when a component has been renamed, for example:

```yaml
attributes:
  - key: db.operation
    type: string
    brief: 'Deprecated, use `db.operation.name` instead.'
    stability: development
    deprecated:
      reason: renamed
      renamed_to: db.operation.name
```

Renames should be used for trivial renames when semantics of the attribute, metric, entity, or another component remained unchanged.

Rename reason MUST NOT be used when anything substantial about the attribute or signal has changed which includes unit or instrument type for metrics or value format for attributes.

#### Obsolete

Use when a component is no longer valid and has no replacement, for example:

```yaml
attributes:
  - key: db.jdbc.driver_classname
    type: string
    brief: 'Removed, no replacement at this time.'
    stability: development
    deprecated:
      reason: obsoleted
      note: >
        Removed, no replacement at this time.
```

#### Uncategorized

For more complex deprecation scenarios:

```yaml
attributes:
  - key: db.connection_string
    type: string
    brief: 'Deprecated, use `server.address`, `server.port` attributes instead.'
    stability: development
    deprecated:
      reason: uncategorized
      note: >
        Replaced by `server.address` and `server.port`.
```

Deprecated structure may be extended in the future to support other reasons.

### Annotations

Annotations provide additional information about the attribute, signal, or enum member. The annotations are recorded as key-value pairs where keys are strings and the values are any YAML value.

Annotations are dynamic in nature and are not controlled by semantic convention tooling. Authors can define arbitrary annotations which could later be used during [code generation](https://github.com/open-telemetry/semantic-conventions/blob/v1.36.0/docs/non-normative/code-generation.md) or [live checks](/crates/weaver_live_check/README.md).

The annotations used by OpenTelemetry semantic conventions are described below:

#### Code Generation Annotations

The `code_generation` annotation controls how code generators should handle the component:

```yaml
metrics:
  - name: http.server.request.duration
    brief: "Duration of HTTP server requests."
    stability: stable
    unit: "s"
    instrument: histogram
    annotations:
      code_generation:
        metric_value_type: double  # Specify the exact type for generated code
```

The `exclude` flag can be used to prevent code generation for problematic items:

```yaml
attributes:
  - key: messaging.client_id
    type: string
    stability: development
    brief: >
      Deprecated, use `messaging.client.id` instead.
    examples: ['client-5', 'myhost@8742@s8083jm']
    deprecated:
      reason: renamed
      renamed_to: messaging.client.id
    annotations:
      code_generation:
        exclude: true  # Skip this attribute during code generation
```

[DocumentStatus]: https://opentelemetry.io/docs/specs/otel/document-status