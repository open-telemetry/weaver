# Weaver Configuration File - `weaver.yaml`

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

# Uncomment this section to specify the configuration of the Jinja template syntax
# and control whitespace behavior.
# Note: The default syntax is strongly recommended.
#template_syntax:
#  block_start: "{%"
#  block_end: "%}"
#  variable_start: "{{"
#  variable_end: "}}"
#  comment_start: "{#"
#  comment_end: "#}"

# Uncomment this section to specify the whitespace behavior of the Jinja template engine.
# For more info, see: https://docs.rs/minijinja/latest/minijinja/syntax/index.html#whitespace-control
# whitespace_control:
#   trim_blocks: true
#   lstrip_blocks: true
#   keep_trailing_newline: true

# Uncomment the following section to specify a list of acronyms that
# will be interpreted by the acronym filter. This is optional.
# acronyms: ["iOS", "HTTP", "API", "SDK", "CLI", "URL", "JSON", "XML", "HTML"]

# Uncomment the following section to specify the configuration of parameters.
# This is optional.
# params:
#  param1: val1
#  param2: val2

# Uncomment the following templates to override the default template
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
```
