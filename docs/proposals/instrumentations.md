# Packages/Instrumentations

This document is a design proposal to add the ability to define packages of signals in semconv which represent what is to be implemented as part of an instrumentation.

## Use cases

1. how we can define requirement levels of metrics, events etc when this requirement changes based on the instrumentation.
2. how can we generate documentation representing what an instrumentation is to provide
3. how can we introduce standardisation to instrumentation scopes

## Defining Instrumentations

The objective is to define a collection of signals which is to be implemented.
Based on this there is 2 approaches which we could offer, with an example of both below for `http.server`:

### Top level

This would explicitly add the signals to the instrumentation with the requirement defined at the instrumentation level

```yaml
instrumentation:
  - name: http.server
    brief: Defines a http server package
    attributes:
      - ref: server.address
        brief: The ip address or dns name which the http request is sent to.
    spans:
      - ref: http.server
        requirement_level: required
    metrics:
      - ref: http.server.request.duration
        requirement_level: required
      - ref: http.server.request.body.size
        requirement_level: recommended
      - ref: http.server.response.body.size
        requirement_level: recommended
```

This change means moving requirement off signal and on to the instrumentation but makes it difficult to manage refinements.


### Adding instrumentation to refinement

This option would overcome the pain points of the previous ie moving the requirement level and provide better support for refinements.

This would be that refinements are now required to contain 1 or more instrumentations listed which acts as the links. 

This definition would look like:

```yaml
instrumentation:
  - name: http.server
    brief: Defines a http server package
    attributes:
      - ref: server.address
        brief: The ip address or dns name which the http request is sent to.
spans:
  - ref: http.server
    requirement_level: required
    instrumentations:
      - http.server
metrics:
  - ref: http.server.request.duration
    requirement_level: required
    instrumentations:
      - http.server
  - ref: http.server.request.body.size
    requirement_level: recommended
    instrumentations:
      - http.server
  - ref: http.server.response.body.size
    requirement_level: recommended
    instrumentations:
      - http.server
```

To be discussed is if the base definitions can optionally contain instrumentations or should the refinement be required.

## Exporting registry  

For the registry, these definitions would follow the other signals where in which the references are resolved.

The only change would be the addition of the instrumentation field on the refinement which is a string as opposed to an array.
This change is possible as each instrumentation in the instrumentations field on the definition would become it's own refinement.

## Functionality

### Defining Instrumentation Attributes

Both options provide the ability for attributes to be defined at the instrumentation level as supported by spec.
To make these attributes consistent for tooling and documentation they should be added to the attributes array on the signal,
with additional fields to indicate where they should be set.

An example of how this refinement would look when exported:

```yaml
span:
  - name: messaging.client
    instrumentation: rabbitmq.client
    attributes:
      - ref: server.address
        brief: The ip address or dns name which the http request is sent to.
        capture_scope:
          type: signal
      - ref: messaging.system
        brief: The messaging system in use
        capture_scope:
          type: instrumentation
```

This approach enables tools like live-check to check if attribute is present in the intended scope and if not,
check if it is at a lower level. For instance a scope attribute is allowed to instead be on the signal but should not be a resource attribute.

## Extensions

### Import instrumentations

When importing a registry to a new project/registry it should be possible to specify the instrumentation to be imported.
This enables a user if instrumenting a rabbitmq client to get everything which is part of that instrumentation without needing to explicitly list all signals.
