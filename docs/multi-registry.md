# Working with Multiple Registries

Weaver supports building telemetry schemas that depend on definitions from other registries. This capability allows you to:

- Reuse definitions from upstream registries (like the official OpenTelemetry Semantic Conventions)
- Create vendor-specific or application-specific telemetry while referencing standard definitions
- Build layered telemetry architectures with clear dependency chains

## Overview

A **multi-registry** setup consists of:

1. **Base registries** - Upstream registries that provide foundational definitions (attributes, metrics, events, entities)
2. **Dependent registries** - Your custom registries that build upon and reference definitions from base registries
3. **Registry manifest** - A `registry_manifest.yaml` file that declares dependencies between registries
4. **Imports** - Declarations in your schema files that specify which groups from dependencies you want to include

## Registry Manifest

Each registry must have a `registry_manifest.yaml` file in its root directory. This file defines the registry's identity and its dependencies on other registries.

### Basic Structure

```yaml
name: my-registry
description: My custom telemetry schema
version: 1.0.0
repository_url: https://example.com/schemas/
dependencies:
  - name: otel
    registry_path: path/to/otel/registry
```

### Required Fields

- **name**: Unique identifier for your registry
- **version**: Semantic version of your registry (also called `semconv_version`)
- **repository_url**: Base URL where schema files are hosted (also called `schema_base_url`)

### Declaring Dependencies

The `dependencies` section lists other registries that your registry depends on. Each dependency has:

- **name**: A short name you'll use to reference this dependency
- **registry_path**: Path to the dependency's registry directory

The `registry_path` can be:

- A relative path: `data/multi-registry/otel_registry`
- An absolute path: `/path/to/otel/registry`
- A URL with optional archive path: `https://github.com/open-telemetry/semantic-conventions/archive/refs/tags/v1.37.0.zip[model]`

> **Note**: Currently, Weaver supports zero or one direct dependency per registry. However, transitive dependencies (dependencies of dependencies) are fully supported, allowing you to create multi-level registry hierarchies. See [issue #604](https://github.com/open-telemetry/weaver/issues/604) for tracking multiple direct dependencies support.

### Example: Three-Level Registry Hierarchy

Here's a practical example showing how registries can form a dependency chain:

**OTEL Registry** (`otel_registry/registry_manifest.yaml`):
```yaml
name: otel
description: OpenTelemetry base definitions
version: 1.0.0
repository_url: https://opentelemetry.io/schemas/
```

**Vendor Registry** (`acme_registry/registry_manifest.yaml`):
```yaml
name: acme
description: Acme vendor-specific definitions
version: 0.1.0
repository_url: https://acme.com/schemas/
dependencies:
  - name: otel
    registry_path: ../otel_registry
```

**Application Registry** (`app_registry/registry_manifest.yaml`):
```yaml
name: app
description: Application-specific telemetry
version: 0.1.0
repository_url: https://app.com/schemas/
dependencies:
  - name: acme
    registry_path: ../acme_registry
```

In this setup:
- The `app` registry depends on `acme`
- The `acme` registry depends on `otel`
- The `app` registry can use definitions from both `acme` and `otel` (transitive dependencies are supported)

## Using Imports

The `imports` section in your schema files specifies which groups from dependent registries you want to include. This allows you to selectively pull in only the definitions you need.

### Import Syntax

Add an `imports` section at the top level of your YAML schema file (alongside `groups`):

```yaml
groups:
  - id: my.custom.group
    type: attribute_group
    # ... group definition ...

imports:
  metrics:
    - example.*
  events:
    - session.start
  entities:
    - gcp.*
```

### Import Categories

Imports are organized by signal type:

- **metrics**: Import metric definitions
- **events**: Import event definitions
- **entities**: Import entity definitions

### Wildcard Patterns

You can use wildcards to import multiple groups at once:

- `example.*` - Imports all groups with names starting with `example.`
- `gcp.*` - Imports all groups with names starting with `gcp.`
- `session.start` - Imports only the specific `session.start` group

### How Imports Work

When you import a group:

1. The group definition from the dependency is included in your resolved schema
2. Any attributes referenced by that group are also included
3. The imported groups can be referenced using `ref` in your custom definitions

### Example: Referencing Imported Definitions

**Base Registry** (`otel_registry/otel_registry.yaml`):
```yaml
groups:
  - id: otel.registry
    type: attribute_group
    attributes:
      - id: error.type
        type: string
        brief: The error type.
        stability: stable

  - id: metric.example.counter
    type: metric
    metric_name: example.counter
    instrument: counter
    unit: "1"
    attributes:
      - ref: error.type
```

**Custom Registry** (`custom_registry/custom_registry.yaml`):
```yaml
groups:
  - id: shared.attributes
    type: attribute_group
    attributes:
      - id: auction.id
        type: int
        brief: The id of the auction.
      - id: auction.name
        type: string
        brief: The name of the auction.
      - ref: error.type  # References attribute from dependency

  - id: metric.auction.bid.count
    type: metric
    metric_name: auction.bid.count
    instrument: counter
    unit: "{bid}"
    attributes:
      - ref: auction.id
      - ref: auction.name
      - ref: error.type  # References attribute from dependency

imports:
  metrics:
    - example.*  # Imports metric.example.counter from dependency
```

**Application Registry** (`app_registry/app_registry.yaml`):
```yaml
groups:
  - id: app.example
    type: attribute_group
    attributes:
      - id: app.name
        type: string
        brief: Name of the application.
      - ref: error.type     # References from transitive dependency (otel)
      - ref: auction.name   # References from direct dependency (custom)

imports:
  metrics:
    - example.*  # Imports from transitive dependencies
```

## The `--include-unreferenced` Flag

By default, Weaver performs **garbage collection** on definitions from dependency registries. This means only definitions that are explicitly referenced (via `ref` or `imports`) are included in the final resolved schema.

The `--include-unreferenced` flag changes this behavior to include **all** definitions from dependencies, whether they're referenced or not.

### Default Behavior (Without `--include-unreferenced`)

```bash
weaver registry resolve my_registry/
```

In this mode:
- ✅ Definitions that are directly referenced via `ref` are included
- ✅ Groups specified in `imports` are included
- ❌ Unreferenced definitions from dependencies are **excluded**

### With `--include-unreferenced`

```bash
weaver registry resolve --include-unreferenced my_registry/
```

In this mode:
- ✅ All definitions from dependencies are included, regardless of references
- ✅ This includes attributes, groups, metrics, events, and entities
- ⚠️ Results in a larger resolved schema

### When to Use Each Mode

**Use default mode (without flag)** when:
- You want a minimal schema with only used definitions
- You're generating code and want to avoid unused definitions
- You want to reduce the size of your resolved schema

**Use `--include-unreferenced`** when:
- You need complete visibility into all available definitions
- You're exploring or documenting what's available in dependencies
- You're building tooling that needs to know about all possible definitions
- You want to generate comprehensive documentation

### Example Impact

Consider this dependency structure where the `otel` registry has:

```yaml
groups:
  - id: otel.registry
    attributes:
      - id: error.type  # Referenced in custom registry

  - id: otel.unused
    attributes:
      - id: unused      # NOT referenced anywhere

  - id: metric.example.counter  # Listed in imports

  - id: event.session.end  # NOT listed in imports
```

**Without `--include-unreferenced`:**
- ✅ `error.type` attribute (referenced via `ref`)
- ✅ `metric.example.counter` (specified in `imports`)
- ❌ `otel.unused` group (not referenced)
- ❌ `event.session.end` (not in imports)

**With `--include-unreferenced`:**
- ✅ All groups and attributes included, even `otel.unused` and `event.session.end`

## Real-World Example: OpenTelemetry Semantic Conventions

Here's how to create a custom registry that depends on the official OpenTelemetry Semantic Conventions:

### 1. Create Registry Manifest

**File**: `model/registry_manifest.yaml`

```yaml
name: my-custom-telemetry
description: Custom telemetry for my application
version: 1.0.0
repository_url: https://my-app.example.com/schemas/
dependencies:
  - name: otel
    registry_path: https://github.com/open-telemetry/semantic-conventions/archive/refs/tags/v1.37.0.zip[model]
```

### 2. Define Custom Attributes

**File**: `model/attributes.yaml`

```yaml
groups:
  - id: registry.example
    type: attribute_group
    brief: Custom application attributes
    attributes:
      - id: example.message
        type: string
        brief: A simple message
        stability: development
        examples: ["Hello, World!"]
      - ref: host.name      # Reference from OTel
      - ref: host.arch      # Reference from OTel
```

### 3. Define Custom Signals

**File**: `model/signals.yaml`

```yaml
groups:
  - id: span.example_message
    type: span
    stability: development
    brief: This span represents a simple message.
    span_kind: client
    attributes:
      - ref: example.message
        requirement_level: required
      - ref: host.name
        requirement_level: required
      - ref: host.arch
        requirement_level: required

  - id: metric.example_counter
    type: metric
    metric_name: example.counter
    stability: development
    brief: A counter of messages processed.
    instrument: counter
    unit: "1"
    attributes:
      - ref: host.name
        requirement_level: required
      - ref: host.arch
        requirement_level: required
```

### 4. Resolve the Schema

```bash
# Minimal schema (only referenced definitions)
weaver registry resolve model/

# Full schema (all definitions from OTel)
weaver registry resolve --include-unreferenced model/
```

## Common Use Cases

### Use Case 1: Vendor Extensions

A cloud provider can extend OTel definitions with vendor-specific attributes:

```yaml
# vendor_registry/registry_manifest.yaml
name: cloud-vendor
version: 1.0.0
repository_url: https://vendor.cloud/schemas/
dependencies:
  - name: otel
    registry_path: https://github.com/open-telemetry/semantic-conventions/archive/refs/tags/v1.37.0.zip[model]
```

```yaml
# vendor_registry/extensions.yaml
groups:
  - id: registry.cloud_vendor
    type: attribute_group
    attributes:
      - id: cloud.vendor.region
        type: string
        brief: Vendor-specific region identifier
      - ref: cloud.platform  # From OTel
      - ref: cloud.region    # From OTel
```

### Use Case 2: Application-Specific Metrics

Applications can import specific metrics they use:

```yaml
# app/model/app_metrics.yaml
imports:
  metrics:
    - http.server.*
    - db.client.*

groups:
  - id: metric.app.requests
    type: metric
    metric_name: app.requests.total
    instrument: counter
    unit: "1"
    attributes:
      - ref: http.request.method  # From imported http metrics
      - ref: app.custom.field     # Custom attribute
```

### Use Case 3: Testing Registries

When testing, you can create isolated test registries:

```yaml
# test_data/test_registry/registry_manifest.yaml
name: test
version: 0.1.0
repository_url: https://test.local/
dependencies:
  - name: base
    registry_path: ../base_registry
```

## Best Practices

1. **Version Dependencies Explicitly**: Use specific version tags in URLs rather than `main` or `latest`
   ```yaml
   registry_path: https://github.com/org/repo/archive/refs/tags/v1.2.3.zip[model]
   ```

2. **Use Wildcard Imports Judiciously**: Prefer specific imports over wildcards when you know exactly what you need
   ```yaml
   # Specific - better for minimal schemas
   imports:
     events:
       - session.start
       - session.end

   # Wildcard - better for exploratory work
   imports:
     events:
       - session.*
   ```

3. **Document Your Dependencies**: Add clear descriptions in your registry manifest
   ```yaml
   description: |
     Custom telemetry extending OTel v1.37.0.
     Adds vendor-specific attributes for auction systems.
   ```

4. **Structure by Concern**: Split definitions across multiple files
   ```
   model/
     registry_manifest.yaml
     attributes.yaml    # Custom attributes
     metrics.yaml       # Custom metrics
     spans.yaml         # Custom spans
   ```

## Troubleshooting

### Issue: Circular Dependencies

**Error**: Registry A depends on Registry B, which depends on Registry A

**Solution**: Restructure your registries to have a clear dependency hierarchy. Consider extracting shared definitions into a common base registry.

### Issue: Missing References

**Error**: Attribute `foo.bar` not found

**Causes**:
1. The attribute is not defined in any dependency
2. The attribute's group is not imported
3. The attribute exists but its group is garbage collected

**Solutions**:
- Add the attribute to an `imports` section
- Use `--include-unreferenced` to verify the attribute exists
- Check that the dependency is correctly declared in `registry_manifest.yaml`

### Issue: URL-based Dependencies Not Loading

**Error**: Cannot load registry from URL

**Solutions**:
- Verify the URL is accessible
- Check that archive paths (in `[brackets]`) point to the correct directory inside the archive
- Ensure the URL points to a registry with a valid `registry_manifest.yaml`

## Command Reference

All Weaver commands that work with registries support the `--include-unreferenced` flag:

```bash
# Resolve a registry
weaver registry resolve [--include-unreferenced] <registry-path>

# Generate code from a registry
weaver registry generate [--include-unreferenced] <registry-path> <template>

# Validate a registry
weaver registry check [--include-unreferenced] <registry-path>

# Update resolved schema
weaver registry update-markdown [--include-unreferenced] <registry-path>

# Calculate statistics on a registry
weaver registry stats [--include-unreferenced] <registry-path>

# Generate diff between registry versions
weaver registry diff [--include-unreferenced] <baseline-path> <head-path>
```

## See Also

- [Registry Overview](registry.md) - Understanding the registry format
- [Define Your Own Telemetry Schema](define-your-own-telemetry-schema.md) - Creating custom schemas
- [Code Generation](codegen.md) - Generating code from registries
- [OpenTelemetry Weaver Examples](https://github.com/open-telemetry/opentelemetry-weaver-examples) - Complete working examples
