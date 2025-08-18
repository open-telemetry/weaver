# Define your own telemetry schema

Weaver allows you to specify your application's signals using a telemetry schema
based on the concept of semantic conventions. These telemetry schemas are also
called custom registries. You can define your own signals or reuse the signals
and attributes defined by the OTEL semantic conventions (url).

To define your schema, you must:

- create a directory that will contain your semantic conventions
- add a `registry_manifest.yaml` file (see structure below)
- add semantic convention files describing the signals that your application may
  produce

From there, you can use Weaver commands to check, resolve, generate code and
documentation, or even control your instrumentation with the `live-check`
command.

## `registry_manifest.yaml` file

This manifest file is used to define the metadata of your custom registry and
the dependencies it has on other registries. The manifest file is required for
the `weaver` tool to recognize your custom registry and to resolve it correctly.

```yaml
name: <custom registry name>
description: <an optional description of the custom registry>
semconv_version: <version of this custom registry>
schema_base_url: <base URL where the registry's schema files are hosted>
dependencies:
  - name: <an alias for the dependency>
    registry_path: <the location of the dependency>
```

> **Current limitations**:
> - The `schema_base_url` field is not currently used by the weaver tool. It is
    intended for future use once telemetry schema v2 is fully specified and
    implemented.
> - Weaver supports a maximum of 10 registry levels without circular
    dependencies. In practice, this is not a limitation, even for complex
    enterprise environments.

Below is an example of a valid `registry_manifest.yaml` file:

```yaml
name: acme
description: This registry contains the semantic conventions for the Acme vendor.
semconv_version: 0.1.0
schema_base_url: https://acme.com/schemas/
dependencies:
  - name: otel
    registry_path: https://github.com/open-telemetry/semantic-conventions/archive/refs/tags/v1.34.0.zip[model]
```

## Semantic conventions files

Semantic conventions are defined in YAML files. OpenTelemetry maintains a
registry of general attributes and signals across many domains, from databases
and messaging to generative AI (see
the [official OTEL semantic conventions](https://opentelemetry.io/docs/specs/semconv/)).

You can define your own semantic conventions or reuse those defined by OTEL. The
example below shows how to define a simple span semantic convention representing
a message sent by a client application.

You can also import existing semantic conventions from other registries, such as
the OTEL semantic conventions, to extend your custom registry.

```yaml
groups:
  - id: span.example_message
    type: span
    stability: development
    brief: This span represents a simple message.
    span_kind: client
    attributes:
      - ref: host.name                 # imported from OTEL semantic conventions
        requirement_level: required    # requirement level redefined locally
      - ref: host.arch                 # imported from OTEL semantic conventions
        requirement_level: required    # requirement level redefined locally

imports:
  metrics:
    - db.*                # import all metrics in the `db` namespace
  entities:
    - gcp.*               # import all entities in the `gcp` namespace
  events:
    - session.start       # import the `session.start` event
```

In the `imports` section, you can specify which metric, event, and entity groups
to import from other registries. You can use a wildcard expression to import all
groups in a namespace or specify individual groups by name.

## Run weaver commands on your schema

To check the validity of your custom registry

```bash
weaver registry check -r <path-to-your-registry>
```

To control the compliance of your instrumentation with your custom registry

```bash
weaver registry live-check --registry <path-to-your-registry>
```

All commands accepting the `-r` or `--registry` parameter can be applied to your
custom registry. It is important to note that some templates are specific to the
OTEL registry. We are working to remove this type of limitation.
