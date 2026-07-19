# Telemetry Schema v1.2.0

```yaml
# This annotated YAML document describes version 1.2.0 of the OpenTelemetry
# telemetry schema structure. This version 1.2.0 is backward compatible with
# version 1.1.0. Version 1.2.0 introduces many new concepts aimed at
# describing:
# - the telemetry signals that an application, service, device, or library can
#   produce (either authored by a developer or produced following a reference
#   resolution mechanism to produce self-contained version).
# - a resolved catalog of attributes and signals defined in a semantic
#   convention registry in an easily exchangeable and consumable form by other
#   tools.
file_format: 1.2.0
schema_url: <same-as-v1.1.0>
# Optional field used to define an inheritance relationship between two
# schemas. The current schema extends the one in the following URL.
# Presence: This field does not exist when the current schema is a resolved
# schema.
extends: <parent-telemetry-schema-url>

# Optional section used to import one or several semantic convention
# registries. 
# Presence: This section does not exist when the current schema is a resolved
# schema.
import_semantic_convention_registries:
  # A semantic convention registry can be imported from a git repository
  - git_url: <git-url-of-the-semantic-conventions-registry>
    # Optional path to the directory containing the semantic convention
    # registry. If not specified, the root of the git repository is used.
    path: <path-in-the-git-repo>
  # A semantic convention registry can also be imported from one or several
  # files composing a registry.
  - url: <url-of-the-semantic-conventions-file>

# Optional section used to describe the attributes of an OpenTelemetry
# resource. This section applies only if the current schema is used to describe
# the signals of a component using a client SDK.
# This section must not contain any external references if the current schema
# is resolved.
# Presence: This section does not exist when the current schema does not belong
# to a deployable component such as an application or a service.
resource:
  # attributes defined locally or inherited from the parent schema (if any) or
  # from the semantic conventions (if any).
  attributes: # List of attribute definitions
    # A new attribute definition
    - id: <attribute-identifier>
      # other field definitions, see semantic convention file format.
    # A attribute definition that overrides a previously defined attribute
    - ref: <attribute-reference>
      # other field definitions, see semantic convention file format.
    # A reference to an attribute defined in the shared catalog within this
    # file.
    - lref: <attribute-local-reference>

# Section used to define the instrumentation library, its version, and the
# schema of OpenTelemetry signals reported by an application, a service, a
# device, or a library.
# Presence: This section doesn't exist when the current schema does not belong
# to an application, a service, a device, or a library.
instrumentation_library:
  name: <instrumentation-library-name>
  version: <instrumentation-library-version>
  # Section describing the telemetry signals produced by the current component
  # (i.e. metrics, logs, events, and spans).
  schema:
    # Declaration of all the univariate metrics
    resource_metrics:
      - metric_name: <metric-id>
        # attributes defined locally or inherited from the parent schema (if any) or
        # from the semantic conventions (if any).
        attributes: # attribute definitions
        # ...
        # other metric fields
        # ...
        tags:
          <tag-key>: <tag-value>
        versions: # optional versioning for the metric.

    # Declaration of all the spans
    resource_spans:
      - span_name: <span-id>
        # attributes defined locally or inherited from the parent schema (if any) or
        # from the semantic conventions (if any).
        attributes: # attribute definitions
        # ...
        # other span fields
        # ...
        tags:
          <tag-key>: <tag-value>
        versions: # optional versioning for the metric.

catalog:
  # The attribute catalog is the place where the fields of attributes are defined
  # precisely. The other sections of the Resolved Telemetry Schema refer to the
  # catalog when they need to "attach" one or more attributes to an OTel entity
  # (e.g., resource, metric, span, ...). Within the catalog, each attribute
  # definition is unique. This does not mean that the id of the attributes is
  # unique within the catalog. It means that the set of fields that make up an
  # attribute is unique. The fact that the id of an attribute is not unique in a
  # resolved schema is related to the overload mechanism supported by the
  # telemetry schema component. This process is ensured at the time of the schema
  # resolution process.
  # Note: A reference to an attribute defined in this catalog is defined in terms
  # of the numerical position of the corresponding attribute in the catalog.
  attributes:
    # Array of fully resolved and qualified attributes
    - name: <fully-qualified-attribute-name> # id of the most recent version
      type: <attribute-type>
      # ...
      # other attributes fields
      # ...

      # This field is used when versioning has been implemented for this
      # attribute. It is calculated by the resolution process to simplify the
      # exploitation of versioning by consumers of resolved telemetry schemas.
      versions:
        <version-number>:
          rename_to: <fully-qualified-attribute-name>


# This optional section contains one or more semantic convention registries of
# attributes, spans, metrics, etc., groups. This section only exists when the
# schema resolution process has been applied to one or more registries (e.g.,
# the official and standard OTel registry, and an internal registry of a
# company complementing that of OTel). The `ref` and 'extend' clauses present
# in the initial registry are all resolved and should therefore no longer
# appear here. Only internal references to the attribute catalog are used.
registries:
  # Registry definition
  - registry_url: <registry-url>
    groups:
      - id: <group-id>
        type: <attribute_group|span|...>
        attributes:
          - <attr-ref-number>  # position in the catalog of attributes
          - <...>
        # ...
        # other group fields (except `ref` and `extends` which have been
        # resolved)
        # ...

        # This optional field tracks the provenance and the various
        # transformations that have been applied to the attribute during the
        # resolution process within the current group. If originally the
        # attribute was defined by a reference with some fields locally
        # overridden, the provenance and override operations will be defined
        # by the lineage. If the attribute comes from an extends clause, then
        # the lineage will contain the provenance and the reference of the
        # extends. The exact definition of the lineage field will be detailed
        # in the OTEP describing the format of the resolved schemas.
        # This field is present only upon request during the resolution process.
        # By default, this field is not present.
        lineage:
          provenance: <url-or-file-where-this-group-was-defined>
          attributes: # will be defined in a future OTEP

# This optional section contains the definition of the resolved telemetry
# schema for each dependency of the currently instrumented component
# (application or library). The schema resolution process collects the resolved
# telemetry schemas of the component's dependencies, merges the attribute
# catalog, and adds the instrumentation library of the dependency with all the
# definitions of metrics, logs, spans it contains while adapting the attribute
# references to point to the local catalog.
dependencies:
  - name: <instrumentation-library-name>
    version: <instrumentation-library-version>
    # The schema details all the metrics, logs, and spans specifically generated
    # by that instrumentation library (i.e. a dependency in this context).
    schema: # same structure as the instrumentation_library section (see above)

# Optional section used to define a list of transformations to apply between
# versions.
# Presence: This section doesn't exist when the current schema is a resolved
# schema.
versions:
  # Same structure as in telemetry schema v1.1.0.
```