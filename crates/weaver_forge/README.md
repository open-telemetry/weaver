# Weaver Forge - Template Engine

- [Introduction](#introduction)
- [Template Directory Structure and Naming Conventions](#template-directory-structure-and-naming-conventions)
- [Configuration File - `weaver.yaml`](#configuration-file---weaveryaml)
- [Jinja Filters](#jinja-filters)
- [Jinja Functions](#jinja-functions)
- [Jinja Tests](#jinja-tests)

## Introduction

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
    go/                     <-- Templates to generate the semantic conventions in Go
      ...
    html/                   <-- Templates to generate the semantic conventions in HTML
      ...
    markdown/               <-- Templates to generate the semantic conventions in markdown
      ... 
    rust/                   <-- Templates to generate the semantic conventions in Rust
      ...  
    go/                     <-- Templates to generate the semantic conventions in Go
      ...
  schema/
    sdk-go/                 <-- Templates to generate a Go Client SDK derived from the telemetry schema
      ...
    sdk-rust/               <-- Templates to generate a Rust Client SDK derived from the telemetry schema
      ...
```

The command `weaver generate registry markdown` will generate the markdown
files based on the templates located in the `templates/registry/markdown`.

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

This mechanism allows the template to dynamically generate the name of the file
to be produced and to organize the generated files in a directory structure of
its choice.

## Configuration File - `weaver.yaml`

The configuration file `weaver.yaml` is optional. It allows configuring the
following options:

```yaml
# Uncomment this section to specify the configuration of the `text_map` filter.
#text_maps:
#  java_types:
#    int: int
#    double: double
#    boolean: boolean
#    string: String
#  java_keys:
#    int: intKey
#    double: doubleKey
#    boolean: booleanKey
#    string: stringKey
    
# Deprecated, please use text_maps instead
# Configuration of the type mapping. This is useful to generate code in a
# specific language. This is optional.
# Example: {{ attribute.type | type_mapping }} will be evaluated as int64
# if the semconv attribute type is int.
#type_mapping:
#  int: int64
#  double: double
#  boolean: bool
#  string: string
#  "int[]": "[]int64"
#  "double[]": "[]double"
#  "boolean[]": "[]bool"
#  "string[]": "[]string"
#  ...

# Configuration of the template engine (optional)
#template_syntax:
#  block_start: "{%"
#  block_end: "%}"
#  variable_start: "{{"
#  variable_end: "}}"
#  comment_start: "{#"
#  comment_end: "#}"

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

## Jinja Filters

All the filters available in the MiniJinja template engine are available (see
this online [documentation](https://docs.rs/minijinja/latest/minijinja/filters/index.html)). 

In addition, OTel Weaver provides a set of custom filters to facilitate the
generation of documentation and code.

The following filters are available:
- `lower_case`: Converts a string to lowercase.
- `upper_case`: Converts a string to UPPERCASE.
- `title_case`: Converts a string to TitleCase.
- `pascal_case`: Converts a string to PascalCase.
- `camel_case`: Converts a string to camelCase.
- `snake_case`: Converts a string to snake_case.
- `screaming_snake_case`: Converts a string to SCREAMING_SNAKE_CASE.
- `kebab_case`: Converts a string to kebab-case.
- `screaming_kebab_case`: Converts a string to SCREAMING-KEBAB-CASE.
- `acronym`: Replaces acronyms in the input string with the full name defined in the `acronyms` section of the `weaver.yaml` configuration file.
- `split_ids`: Splits a string by '.' creating a list of nested ids.
- `type_mapping`: Converts a semantic convention type to a target type (see weaver.yaml section `type_mapping`).
- `comment_with_prefix(prefix)`: Outputs a multiline comment with the given prefix.
- `flatten`: Converts a List of Lists into a single list with all elements.
e.g. \[\[a,b\],\[c\]\] => \[a,b,c\]
- `attribute_sort`: Sorts a list of `Attribute`s by requirement level, then name.
- `metric_namespace`: Converts registry.{namespace}.{other}.{components} to {namespace}.
- `attribute_registry_file`: Converts registry.{namespace}.{other}.{components} to attributes-registry/{namespace}.md (kebab-case namespace).
- `attribute_registry_title`: Converts registry.{namespace}.{other}.{components} to {Namespace} (title case the namespace).
- `attribute_registry_namespace`: Converts metric.{namespace}.{other}.{components} to {namespace}.
- `attribute_namespace`: Converts {namespace}.{attribute_id} to {namespace}.
- `required`: Filters a list of `Attribute`s to include only the required attributes. The "conditionally_required" attributes are not returned by this filter.
- `not_required`: Filters a list of `Attribute`s to only include non-required attributes. The "conditionally_required" attributes are returned by this filter.
- `markdown_to_html`: Converts a markdown string to an HTML string.
- `text_map`: Converts an input into a string based on the `text_maps` section of the `weaver.yaml` configuration file and a named text_map.
- `ansi_black`: Format a text using the black ansi code.
- `ansi_red`: Format a text using the red ansi code.
- `ansi_green`: Format a text using the green ansi code.
- `ansi_yellow`: Format a text using the yellow ansi code.
- `ansi_blue`: Format a text using the blue ansi code.
- `ansi_magenta`: Format a text using the magenta ansi code.
- `ansi_cyan`: Format a text using the cyan ansi code.
- `ansi_white`: Format a text using the white ansi code.
- `ansi_bright_black`: Format a text using the bright black ansi code.
- `ansi_bright_red`: Format a text using the bright red ansi code.
- `ansi_bright_green`: Format a text using the bright green ansi code.
- `ansi_bright_yellow`: Format a text using the bright yellow ansi code.
- `ansi_bright_blue`: Format a text using the bright blue ansi code.
- `ansi_bright_magenta`: Format a text using the bright magenta ansi code.
- `ansi_bright_cyan`: Format a text using the bright cyan ansi code.
- `ansi_bright_white`: Format a text using the bright white ansi code.
- `ansi_bg_black`: Format a text using the black background ansi code.
- `ansi_bg_red`: Format a text using the red background ansi code.
- `ansi_bg_green`: Format a text using the green background ansi code.
- `ansi_bg_yellow`: Format a text using the yellow background ansi code.
- `ansi_bg_blue`: Format a text using the blue background ansi code.
- `ansi_bg_magenta`: Format a text using the magenta background ansi code.
- `ansi_bg_cyan`: Format a text using the cyan background ansi code.
- `ansi_bg_white`: Format a text using the white background ansi code.
- `ansi_bg_bright_black`: Format a text using the bright black background ansi code.
- `ansi_bg_bright_red`: Format a text using the bright red background ansi code.
- `ansi_bg_bright_green`: Format a text using the bright green background ansi code.
- `ansi_bg_bright_yellow`: Format a text using the bright yellow background ansi code.
- `ansi_bg_bright_blue`: Format a text using the bright blue background ansi code.
- `ansi_bg_bright_magenta`: Format a text using the bright magenta background ansi code.
- `ansi_bg_bright_cyan`: Format a text using the bright cyan background ansi code.
- `ansi_bg_bright_white`: Format a text using the bright white background ansi code.
- `ansi_bold`: Format a text using the bold ansi code.
- `ansi_italic`: Format a text using the italic ansi code.
- `ansi_underline`: Format a text using the underline ansi code.
- `ansi_strikethrough`: Format a text using the strikethrough ansi code.

> Please open an issue if you have any suggestions for new filters. They are easy to implement.

## Jinja Functions

All the functions available in the MiniJinja template engine are available (see 
this online [documentation](https://docs.rs/minijinja/latest/minijinja/functions/index.html)).

Right now, OTel Weaver does not provide any custom functions but feel free to
open an issue if you have any suggestions. They are easy to implement.

## Jinja Tests

All the tests available in the MiniJinja template engine are available (see
this online [documentation](https://docs.rs/minijinja/latest/minijinja/tests/index.html)).

In addition, OTel Weaver provides a set of custom tests to facilitate the
generation of assets.

- `stable`: Tests if an `Attribute` is stable.
- `experimental`: Tests if an `Attribute` is experimental.
- `deprecated`: Tests if an `Attribute` is deprecated.

> Please open an issue if you have any suggestions for new tests. They are easy to implement.