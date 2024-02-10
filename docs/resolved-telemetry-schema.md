# Resolved Telemetry Schema

A Resolved Telemetry Schema is the outcome of the schema resolution process.
This process involves taking the entire hierarchy of Telemetry Schemas and
Semantic Convention Registries and applying a set of rules to resolve overrides
and eliminate external references. The key design principles to be followed in
the definition of the Resolved Telemetry Schema are:

* **Self-contained**: No external references are allowed. This artifact contains
  everything required to determine what an application or a library produces in
  terms of telemetry.
* **Easy to exchange**: This artifact must be easily accessible from a web
  server via a URL. This artifact must be small and avoid the repetition of
  definitions.
* **Easy to parse**: A widespread and well-defined format should be preferred.
  JSON is an example of such a format.
* **Easy to interpret**: The internal structure of this artifact must be
  straightforward to avoid any misinterpretation and must be efficient.
* **Platform- and Language-agnostic**: This artifact must be independent of any
  platform architectures and programming languages.

The following diagram describes two main use cases for the Resolved Telemetry
Schema. The key points to remember are: 1) both use cases result in a Resolved
Telemetry Schema, 2) Resolved Telemetry Schemas serve as the mechanism for
distributing Telemetry Schemas throughout the entire ecosystem, and 3) Resolved
Telemetry Schemas would replace/augment existing SchemaURL.

![Use cases](./images/0240-otel-weaver-use-cases.svg)

The main components of a Resolved Telemetry Schema are illustrated in the
diagram below. The 'OTel Weaver' tool is used to create these schemas. It can
also extend an existing schema or import a Semantic Convention Registry.
Resolved Telemetry Schema serves as a key mechanism for interoperability,
feeding various external tools, including SDK generators, documentation
generators, policy enforcers, and more.

![Resolved Telemetry Schema](./images/0240-otel-weaver-resolved-schema.svg)

The internal catalog is used to define all the attributes and metrics in this
artifact. This design allows for the reuse of the same attributes or metrics
multiple times in different signals and different instrumentation libraries. It
is expected to be a very common pattern to reuse the same subset of attributes
or metrics across several signals and libraries.

## Structure

The structure of the resolved telemetry schema is given here as an example and
corresponds to the author's vision (not yet validated) of what the structure of
a resolved telemetry schema could be. The definitive structure and format of
this resolved schema will be discussed and finalized later in a dedicated OTEP.

```yaml
# This file describes the general logical structure envisioned to date for a
# resolved telemetry schema. The precise definition of the structure and format
# of a resolved telemetry schema is the subject of a dedicated OTEP, which does
# not yet exist at the time of writing the current OTEP. The format used here
# is YAML, but another format such as JSON, Protobuf, or another could
# ultimately be chosen based on considerations of efficiency and ease of
# integration.
# The telemetry metadata described in this file is self-contained and describes
# the entirety of telemetry metadata for either:
# - one or more semantic convention registries
# - an application or a service
# - a library
file_format: 1.2.0
schema_url: <url-of-the-current-schema>

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

# The resource field is defined when the resolved schema is that of an
# application (as opposed to that of a library). The resource field contains a
# list of local references to attributes defined in the shared catalog within
# this file.
resource:
  attributes:
    - <attr-ref-number>  # position in the catalog of attributes

# This optional section defines the instrumentation library, its version, and
# the schema of OTel entities reported by this instrumentation library. This
# section is mandatory if the origin of this resolved schema is a telemetry
# schema component (i.e. application or library). Therefore, this section does
# not exist for the resolution of a registry.
instrumentation_library:
  name: <instrumentation-library-name>
  version: <instrumentation-library-version>
  # The schema details all the metrics, logs, and spans specifically generated
  # by that instrumentation library.
  schema:
    # Declaration of all the univariate metrics
    resource_metrics:
      - metric_name: <metric-id>
        attributes:
          - <attr-ref-number>  # position in the catalog of attributes
        # ...
        # other metric fields
        # ...
        tags:
          <tag-key>: <tag-value>
        versions: # optional versioning for the metric.

    # Declaration of all the spans
    resource_spans:
      - span_name: <span-id>
        attributes:
          - <attr-ref-number>  # position in the catalog of attributes
        # ...
        # other span fields
        # ...
        tags:
          <tag-key>: <tag-value>
        versions: # optional versioning for the span.

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
```
