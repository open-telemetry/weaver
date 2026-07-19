# Registry

A registry defines telemetry using a schema.

The official OTel Semantic Conventions are defined in a registry located at [open-telemetry/semantic-conventions/model](https://github.com/open-telemetry/semantic-conventions/tree/main/model).

## Format

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
