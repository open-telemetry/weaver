# Weaver Configuration File - `weaver.yaml`

## Structure

The following options can be configured in the `weaver.yaml` file:

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

# Configuration File Loading Order and Overriding Rules

In the simplest case, a configuration file named `weaver.yaml` is searched for by
the tool within the folder containing the templates. 

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

## Example

Imagine we have two slightly different variants for generating the semconv Python code.

```
templates
├── registry
│   ├── python
│   │   ├── weaver.yaml
│   │   ├── attribute_group.md.j2
│   │   └── ...
│   └── python-incubation
│   │   ├── weaver.yaml
│   │   ├── attribute_group.md.j2
│   │   └── ...
│   └── weaver.yaml
```

The file located in `templates/registry/weaver.yaml` will be loaded first, followed by
`templates/registry/python/weaver.yaml` if the target is `python`.

Similarly, the file located in `templates/registry/weaver.yaml` will be loaded first,
followed by `templates/registry/python-incubation/weaver.yaml` if the target is
`python-incubation`.

To share a list of acronyms between the two variants, you can define the list in
`templates/registry/weaver.yaml`.

```yaml
acronyms: ["iOS", "HTTP", "API", "SDK", "CLI", "URL", "JSON", "XML", "HTML"]
```

This list will be automatically inherited by both variants except if the list
is redefined in the variant's `weaver.yaml` file.