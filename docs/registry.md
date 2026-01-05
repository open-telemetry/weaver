# Registry

A registry defines telemetry using a schema.

The official OTel Semantic Conventions are defined in a registry located at [open-telemetry/semantic-conventions/model](https://github.com/open-telemetry/semantic-conventions/tree/main/model).

## Schema Versions

Weaver supports two schema versions:

- **V1 (default)**: The original, stable schema format using `groups`
- **V2 (alpha)**: The next-generation schema format with improved structure

Use the `--v2` flag with weaver commands to work with V2 schemas.

> **Note**: V2 schema is currently in Alpha status and under active development. See [semconv-syntax.v2.md](/schemas/semconv-syntax.v2.md) for the complete specification.

## V1 Format (Default)

Full reference at https://github.com/open-telemetry/weaver/blob/main/schemas/semconv-syntax.md

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/open-telemetry/weaver/refs/heads/main/schemas/semconv.schema.json
groups:
- id: string
  type: metric | span | event | attribute_group | ...
  stability: development | stable | ...
  brief: string
  ...
```

#### Metrics

```yaml
- id: metric.container.uptime
  type: metric
  metric_name: container.uptime
  stability: development
  brief: "The time the container has been running"
  instrument: gauge
  unit: s
```

#### Traces

```yaml
- id: span.http.client
  type: span
  extends: attributes.http.client
  stabilit: stable
  brief: "This span represents an outbound HTTP request"
  attributes:
    - ref: http.request.method
      sampling_relevant: true
```

#### Attributes

Attributes can be defined either inline at the metric/span/etc. or centrally in a group:

```yaml
- id: registry.http
  type: attribute_group
  brief: "Describes HTTP attributes"
  attributes:
  - id: http.request.method
    stability: stable
    type:
      members:
      - id: get
        value: GET
      - id: head
        value: HEAD
```

Once defined, they can be referred to by id:

```yaml
- id: metric.http.server.active_requests
  type: metric
  metric_name: http.server.active_requests
  instrument: updowncounter
  unit: "{request}"
  attributes:
  - ref: http.request.method    # defined in registry.http above
    requirement_level: required
```

## V2 Format (Alpha)

Full reference at [semconv-syntax.v2.md](/schemas/semconv-syntax.v2.md)

V2 schema uses a different top-level structure with `version: "2"` and direct signal type definitions instead of groups.

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/open-telemetry/weaver/refs/heads/main/schemas/semconv.schema.v2.json
version: "2"
```

### Attributes

Attributes are defined at the top level in an `attributes` section:

```yaml
version: "2"
attributes:
  - key: server.address
    stability: stable
    type: string
    brief: The domain name or IP address of the server.
    examples: ['example.com', '10.1.2.80']
  - key: server.port
    stability: stable
    type: int
    brief: The port number of the server.
    examples: [80, 8080, 443]
```

### Metrics

Metrics are defined directly in a `metrics` section:

```yaml
version: "2"
metrics:
  - name: container.uptime
    stability: development
    brief: "The time the container has been running"
    instrument: gauge
    unit: s
```

### Spans

Spans are defined in a `spans` section with references to attributes:

```yaml
version: "2"
spans:
  - name: http.client
    stability: stable
    brief: "This span represents an outbound HTTP request"
    attributes:
      - ref: http.request.method
        requirement_level: required
      - ref: server.address
        requirement_level: required
```

### Events

Events are defined in an `events` section:

```yaml
version: "2"
events:
  - name: exception
    stability: stable
    brief: "This event represents an exception"
    attributes:
      - ref: exception.type
        requirement_level: required
```

### Using V2 with Weaver

To work with V2 schemas, use the `--v2` flag with weaver commands:

```bash
# Check a V2 registry
weaver registry check --v2 -r ./my-v2-registry

# Generate artifacts from a V2 registry
weaver registry generate --v2 -r ./my-v2-registry my-target

# Resolve a V2 registry
weaver registry resolve --v2 -r ./my-v2-registry
```

