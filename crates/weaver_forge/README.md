# Weaver Forge - Template Engine

OTel Weaver is capable of generating documentation or code from a semantic
convention registry or a telemetry schema (phase 2). To do this,
OTel Weaver uses a template engine compatible with the Jinja2 syntax (see the
[MiniJinja](https://github.com/mitsuhiko/minijinja) project for more details).
A set of filters, functions, tests, and naming conventions have been added to
the classic Jinja logic to make the task easier for template authors.

The following diagram illustrates the documentation and code generation pipeline
using the OTel Weaver tool:

![Weaver Forge](images/artifact-generation-pipeline.svg)

## Template Directory Structure and Naming Conventions

By default, the OTel Weaver tool expects to find a templates directory in the
current directory.

```plaintext
templates/
  registry/
    markdown/              <-- All the files in this directory are optional 
      attribute_group.md   <-- will be evaluated for each attribute group
      attribute_groups.md  <-- will be evaluated once with all attribute groups
      event.md             <-- will be evaluated for each event
      events.md            <-- will be evaluated once with all events
      group.md             <-- will be evaluated for each group
      groups.md            <-- will be evaluated once with all groups
      metric.md            <-- will be evaluated for each metric
      metrics.md           <-- will be evaluated once with all metrics
      metric_group.md      <-- will be evaluated for each metric group
      metric_groups.md     <-- will be evaluated once with all metric groups
      registry.md          <-- will be evaluated once with the entire registry
      resource.md          <-- will be evaluated for each resource
      resources.md         <-- will be evaluated once with all resources
      scope.md             <-- will be evaluated for each scope
      scopes.md            <-- will be evaluated once with all scopes
      span.md              <-- will be evaluated for each span
      spans.md             <-- will be evaluated once with all spans
      weaver.yaml          <-- weaver configuration file (optional)
      any_other_name.md    <-- will be evaluated once with the entire registry
      any_sub_dir/         <-- By default outputs to the same directory structure
        any_file.md        <-- will be evaluated once with the entire registry
    html/
      ...  
  schema/
    sdk-go/
      ...
    sdk-rust/
      ...
```

The command `weaver generate registry markdown` will generate the markdown
files.

When the name of a file (excluding the extension) matches a recognized pattern
(e.g., attribute_group, groups, ...), OTel Weaver extracts the objects from the
registry and passes them to the template at the time of its evaluation.
Depending on the nature of the pattern, the template is evaluated as many times
as there are objects that match or only once if the pattern corresponds to a
set of objects. By default, the name of the file that will be generated from
the template will be that of the template, but it is possible within the
template to dynamically redefine the name of the produced file.

For example, the following snippet redefine the name of the file that will be
produced from the template:

```jinja
{%- set file_name = group.id | file_name -%}
{{- template.set_file_name("span/" ~ file_name ~ ".md") -}}
```

## Configuration File

The configuration file `weaver.yaml` is optional. It allows configuring the
following options:

```yaml
# Configuration of the naming convention filters. This is optional.
# Example: {{ group.id | file_name }} will be evaluated as group_id
file_name: snake_case
function_name: PascalCase
arg_name: camelCase
struct_name: PascalCase
field_name: PascalCase

# Configuration of the type mapping. This is useful to generate code in a
# specific language. This is optional.
# Example: {{ attribute.type | type_mapping }} will be evaluated as int64
# if the semconv attribute type is int.
type_mapping:
  int: int64
  double: double
  boolean: bool
  string: string
  "int[]": "[]int64"
  "double[]": "[]double"
  "boolean[]": "[]bool"
  "string[]": "[]string"
  # other mappings...

# Configuration of the template engine (optional)
template_syntax:
  block_start: "{%"
  block_end: "%}"
  variable_start: "{{"
  variable_end: "}}"
  comment_start: "{#"
  comment_end: "#}"

# Please uncomment the following section to specify a list of acronyms that
# will be interpreted by the acronym filter. This is optional.
# acronyms: ["iOS", "HTTP", "API", "SDK", "CLI", "URL", "JSON", "XML", "HTML"]

# Please uncomment the following templates to override the default template
# mapping. Each template mapping specifies a jaq filter (compatible with jq)
# to apply to every file matching the pattern. The application_mode specifies
# how the template should be applied. The application_mode can be `each` or
# `single`. The `each` mode will evaluate the template for each object selected
# by the jaq filter. The `single` mode will evaluate the template once with all
# the objects selected by the jq filter.
#
# Note: jaq is a Rust reimplementation of jq. Most of the jq filters are
# supported. For more information, see https://github.com/01mf02/jaq
#
# templates:
#  - pattern: "**/registry.md"
#    filter: "."
#    application_mode: single
#  - pattern: "**/attribute_group.md"
#    filter: ".groups[] | select(.type == \"attribute_group\")"
#    application_mode: each
#  - pattern: "**/attribute_groups.md"
#    filter: ".groups[] | select(.type == \"attribute_group\")"
#    application_mode: single
#  - pattern: "**/event.md"
#    filter: ".groups[] | select(.type == \"event\")"
#    application_mode: each
#  - pattern: "**/events.md"
#    filter: ".groups[] | select(.type == \"event\")"
#    application_mode: single
#  - pattern: "**/group.md"
#    filter: ".groups[] | select(.type == \"group\")"
#    application_mode: each
#  - pattern: "**/groups.md"
#    filter: ".groups[] | select(.type == \"group\")"
#    application_mode: single
#  - pattern: "**/metric.md"
#    filter: ".groups[] | select(.type == \"metric\")"
#    application_mode: each
#  - pattern: "**/metrics.md"
#    filter: ".groups[] | select(.type == \"metric\")"
#    application_mode: single
#  - pattern: "**/metric_group.md"
#    filter: ".groups[] | select(.type == \"metric_group\")"
#    application_mode: each
#  - pattern: "**/metric_groups.md"
#    filter: ".groups[] | select(.type == \"metric_group\")"
#    application_mode: single
#  - pattern: "**/resource.md"
#    filter: ".groups[] | select(.type == \"resource\")"
#    application_mode: each
#  - pattern: "**/resources.md"
#    filter: ".groups[] | select(.type == \"resource\")"
#    application_mode: single
#  - pattern: "**/scope.md"
#    filter: ".groups[] | select(.type == \"scope\")"
#    application_mode: each
#  - pattern: "**/scopes.md"
#    filter: ".groups[] | select(.type == \"scope\")"
#    application_mode: single
#  - pattern: "**/span.md"
#    filter: ".groups[] | select(.type == \"span\")"
#    application_mode: each
#  - pattern: "**/spans.md"
#    filter: ".groups[] | select(.type == \"span\")"
#    application_mode: single
```

Supported case converters:
- lowercase
- UPPERCASE
- PascalCase
- camelCase
- snake_case
- SCREAMING_SNAKE_CASE
- kebab-case
- SCREAMING-KEBAB-CASE

## Custom Filters

All the filters available in the MiniJinja template engine are available. In
addition, OTel Weaver provides a set of custom filters to facilitate the
generation of assets.

The following filters are available:
- `file_name`: Converts a string to a file name.
- `function_name`: Converts a string to a function name.
- `arg_name`: Converts a string to an argument name.
- `struct_name`: Converts a string to a struct name.
- `field_name`: Converts a string to a field name.
- `type_mapping`: Converts a semantic convention type to a language type.
- `lower_case`: Converts a string to lowercase.
- `upper_case`: Converts a string to UPPERCASE.
- `title_case`: Converts a string to TitleCase.
- `pascal_case`: Converts a string to PascalCase.
- `camel_case`: Converts a string to camelCase.
- `snake_case`: Converts a string to snake_case.
- `screaming_snake_case`: Converts a string to SCREAMING_SNAKE_CASE.
- `kebab_case`: Converts a string to kebab-case.
- `screaming_kebab_case`: Converts a string to SCREAMING-KEBAB-CASE.
- `acronym`: Replaces acronyms in the input string with the full name defined
in the `acronyms` section of the `weaver.yaml` configuration file.
- `split_ids`: Splits a string by '.' creating a list of nested ids.
- `flatten`: Converts a List of Lists into a single list with all elements.
e.g. \[\[a,b\],\[c\]\] => \[a,b,c\]
- `attribute_sort`: Sorts a list of `Attribute`s by requirement level, then name.
- `metric_namespace`: Converts registry.{namespace}.{other}.{components} to {namespace}.
- `attribute_registry_file`: Converts registry.{namespace}.{other}.{components} to attributes-registry/{namespace}.md (kebab-case namespace).
- `attribute_registry_title`: Converts registry.{namespace}.{other}.{components} to {Namespace} (title case the namespace).
- `attribute_registry_namespace`: Converts metric.{namespace}.{other}.{components} to {namespace}.
- `attribute_namespace`: Converts {namespace}.{attribute_id} to {namespace}.

> Note 1: This project uses the [convert_case](https://crates.io/crates/convert_case)
> crate to convert strings to different cases. 

> Note 2: Other filters might be introduced in the future.

## Custom Functions

All the functions available in the MiniJinja template engine are available. In
addition, OTel Weaver provides a set of custom functions to facilitate the
generation of assets.

Not yet implemented.

## Custom Tests

All the tests available in the MiniJinja template engine are available. In
addition, OTel Weaver provides a set of custom tests to facilitate the
generation of assets.

- `stable`: Tests if an `Attribute` is stable.
- `experimental`: Tests if an `Attribute` is experimental.
- `deprecated`: Tests if an `Attribute` is deprecated.

> Note: Other tests might be introduced in the future.