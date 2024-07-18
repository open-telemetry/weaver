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

By default, Weaver expects to find the `templates/` directory in the current directory
with the following structure. The location of this directory can be redefined using
the `-t` or `--templates` CLI parameter.

```plaintext
templates/
  registry/                 <-- All templates related to the semantic convention registries
    go/                     <-- Templates to generate the semantic conventions in Go
      ...
    html/                   <-- Templates to generate the semantic conventions in HTML
      ...
    markdown/               <-- Templates to generate the semantic conventions in markdown
      ...
    rust/                   <-- Templates to generate the semantic conventions in Rust
      ...
  schema/
    sdk-go/                 <-- Templates to generate a Go Client SDK derived from the telemetry schema
      ...
    sdk-rust/               <-- Templates to generate a Rust Client SDK derived from the telemetry schema
      ...
```

The command `weaver generate registry rust` will generate the rust
files based on the templates located in the `templates/registry/rust`.

By default, the name of the file that will be generated from
the template will be that of the template, but it is possible within the
template to dynamically redefine the name of the produced file.

For example, the following snippet redefine the name of the file that will be
produced from the template:

```jinja
{%- set file_name = group.id | snake_case -%}
{{- template.set_file_name("span/" ~ file_name ~ ".md") -}}
...
rest of the template
...
```

This mechanism allows the template to dynamically generate the name of the file
to be produced and to organize the generated files in a directory structure of
its choice.

## Configuration File - `weaver.yaml`

In the simplest case, a configuration file named `weaver.yaml` is searched for by
the tool within the folder containing the templates. The syntax of this configuration
file is described here [Weaver Configuration File](/docs/weaver-config.md).

It is possible to utilize the hierarchy of folders containing the targets to share
segments of the configuration common to all targets. Similarly, you can define
Weaver configuration segments in your home directory, i.e., $HOME/.weaver/weaver.yaml.

By default, the `weaver.yaml` files are loaded in the following order:

- $HOME/.weaver/weaver.yaml
- /weaver.yaml, all intermediate directories containing a `weaver.yaml` file up to the
`templates/registry/<target>` directory.
- `templates/registry/<target>/weaver.yaml`

The last configuration file loaded will override the previous ones.

For the most complex cases, it is possible to define explicitly the list configuration
files to load using the `--config` CLI n-ary parameter.

## JQ Filters

Each template present in the `templates/registry/<target>` directory can be associated
with a JQ filter that will be applied to the resolved semconv registry before being
delivered to the template in the `ctx` variable. The definition of the filters follows
the following syntax:

```yaml
templates:
 - pattern: "**/attributes.j2"
   filter: semconv_grouped_attributes
   application_mode: each
 - pattern: "**/metrics.j2"
   filter: semconv_grouped_metrics
   application_mode: each
 - ...
```

In this example, the `attributes.j2` and `metrics.j2` templates are associated with the
`semconv_grouped_attributes` and `semconv_grouped_metrics` JQ filters respectively. These
filters are applied to each object selected by the JQ filter before being delivered to the
template. `semconv_grouped_attributes` returns an array of objects containing the attributes
grouped by namespace. The `application_mode` is set to `each` so that the template is
applied to each object in the array, i.e., to each group of attributes for a given namespace.

A series of JQ filters dedicated to the manipulation of semantic conventions registries is
available to template authors.

**Process Registry Attributes**

The following JQ filter extracts the registry attributes from the resolved registry and
returns a list of registry attributes grouped by namespace and sorted by attribute names.

```yaml
templates:
  - pattern: attributes.j2
    filter: semconv_grouped_attributes
    application_mode: each
```

The output of the JQ filter has the following structure:

```json5
[
  {
    "namespace": "user_agent",
    "attributes": [
      {
        "brief": "Value of the HTTP User-Agent",
        "examples": [ ... ],
        "name": "user_agent.original",
        "namespace": "user_agent",
        "requirement_level": "recommended",
        "stability": "stable",
        "type": "string",
        // ... other fields
      }, 
      // ... other attributes in the same namespace
    ]
  },
  // ... other namespaces
]
```

The `semconv_grouped_attributes` function also supports options to exclude specified namespaces
or specific stability levels. The following syntax is supported:

```yaml
templates:
  - pattern: attributes.j2
    filter: >
      semconv_grouped_attributes({
        "exclude_namespace": ["url", "network"], 
        "exclude_stability": ["experimental"]
      })
    application_mode: each
```

The structure of the output of `semconv_grouped_attributes` with these options is exactly the
same as without the options. The JSON object passed as a parameter describes a series of
options that can easily be extended if needed. Each of these options is optional.

Technically, the `semconv_grouped_attributes` function is a combination of two semconv
JQ functions:

```jq
def semconv_grouped_attributes($options):
    semconv_attributes($options)
    | semconv_group_attributes_by_namespace;

def semconv_grouped_attributes: semconv_grouped_attributes({});
```

The `semconv_attributes` function extracts the registry attributes and applies the given options.
The `semconv_group_attributes_by_namespace` function groups the attributes by namespace. It's
possible to combine these two functions with your own JQ filters if needed.

**Process Metrics**

The following JQ filter extracts the metrics from the resolved registry, sorted by group
namespace and sorted by metric names.

```yaml
templates:
  - pattern: metrics.j2
    filter: semconv_grouped_metrics
    application_mode: each
```

The output of the JQ filter has the following structure:

```json5
[
  {
    "namespace": "jvm",
    "metrics": [
      {
        "attributes": [ ... ],
        "brief": "Recent CPU utilization for the process as reported by the JVM.",
        "id": "metric.jvm.cpu.recent_utilization",
        "instrument": "gauge",
        "metric_name": "jvm.cpu.recent_utilization",
        "namespace": "jvm",
        "note": "The value range is [0.0,1.0]. ...",
        "stability": "stable",
        "type": "metric",
        "unit": "1",
        // ... other fields
      },
      // ... other metrics in the same namespace
    ]
  },
  // ... other namespaces
]
```

The same options are supported by `semconv_grouped_metrics`, as shown in the following example:

```yaml
templates:
  - pattern: metrics.j2
    filter: >
      semconv_grouped_metrics({
        "exclude_namespace": ["url", "network"], 
        "exclude_stability": ["experimental"]
      })
    application_mode: each
```

**Other signals**

The pattern is used for other signals and OTEL entities:
- `semconv_grouped_resources`
- `semconv_grouped_scopes`
- `semconv_grouped_spans`
- `semconv_grouped_events`

All the `semconv_grouped_<...>` functions are the composition of two functions:
`semconv_<...>` and `semconv_group_<...>_by_namespace`.

> Note: JQ is a language for querying and transforming structured data. For more
> information, see [JQ Manual](https://jqlang.github.io/jq/manual/). The
> integration into Weaver is done through the Rust library `jaq`, which is a
> reimplementation of JQ in Rust. Most JQ filters are supported. For more
> information, see [jaq GitHub repository](https://github.com/01mf02/jaq).

## Global Variables

All templates have access to the following global variables:

- `ctx`: The context object that contains the resolved registry or the output of
the JQ filter if defined in the `weaver.yaml` configuration file.
- `params`: The parameters defined in the `weaver.yaml` configuration file or overridden
by the command line `--param`, `-D`, or `--params` arguments.
- `template`: An object exposing the `set_file_name` method to redefine the name of the
file that will be produced from the template.

In the following example, the parameter `incubating` is passed via the command line:

```shell
weaver registry generate --param incubating=true <target> <output-dir>
```

The `weaver.yaml` configuration file can specify default values for the parameters and can also
access the parameters in the JQ filters:

```yaml
params:
  incubating: false
  registry_prefix: "registry."

templates:
  - pattern: <glob-pattern>
    filter: >
      if $incubating then
        .groups
          | map(select(.type == "attribute_group"))
          | map(select(.id | startswith($registry_prefix)))
          | map({ id: .id, group_id: .id | split(".") | .[1], attributes: .attributes })
          | group_by(.group_id)
          | map({ id: .[0].group_id, attributes: [.[].attributes[]] | sort_by(.id), output: "_incubating/attributes/", stable_package_name: "opentelemetry.semconv.attributes" })
          | map(select(.id as $id | any($excluded[]; . == $id) | not))
          | map(select(.attributes | length > 0))
      else
        empty
      end
      application_mode: single | each
```

Jinja templates can also access the parameters:

```jinja
...
{% if params.incubating %}
... generate incubating code ...
{% endif %}
...
```

## Jinja Filters

All the filters available in the MiniJinja template engine are available (see
this online [documentation](https://docs.rs/minijinja/latest/minijinja/filters/index.html)) and the [py_compat](https://github.com/mitsuhiko/minijinja/blob/e8a7ec5198deef7638267f2667714198ef64a1db/minijinja-contrib/src/pycompat.rs) compatibility extensions
that are also enabled in Weaver.

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
- `capitalize_first`: Capitalizes the first letter of a string.
- `kebab_case_const`: Generates kebab-case constants which follow semantic convention namespacing rules (underscores are ignored, but . is meaningful).
- `pascal_case_const`: Generates PascalCase constants which follow semantic convention namespacing rules (underscores are ignored, but . is meaningful).
- `camel_case_const`: Generates camelCase constants which follow semantic convention namespacing rules (underscores are ignored, but . is meaningful).
- `snake_case_const`: Generates snake_case constants which follow semantic convention namespacing rules (underscores are ignored, but . is meaningful).
- `screaming_snake_case_const`: Generates SCREAMING_SNAKE_CASE constants which follow semantic convention namespacing rules (underscores are ignored, but . is meaningful).
- `acronym`: Replaces acronyms in the input string with the full name defined in the `acronyms` section of the `weaver.yaml` configuration file.
- `split_id`: Splits a string by '.' creating a list of nested ids.
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
- `instantiated_type`: Filters a type to return the instantiated type.
- `enum_type`: Filters a type to return the enum type or an error if the type is not an enum.
- `markdown_to_html`: Converts a markdown string to an HTML string.
- `map_text`: Converts an input into a string based on the `text_maps` section of the `weaver.yaml` configuration file
and a named text_map. The first parameter is the name of the text_map (required). The second parameter is the default
value if the name of the text map or the input are not found in the `text_maps` section (optional).
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
- `enum`: Tests if an attribute has an enum type.
- `simple_type`: Tests if a type is a simple type (i.e.: string | string[] | int | int[] | double | double[] | boolean | boolean[]).
- `template_type`: Tests if a type is a template type (i.e.: template[]).
- `enum_type`: Tests if a type is an enum type.

> Please open an issue if you have any suggestions for new tests. They are easy to implement.
