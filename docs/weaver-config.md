# Weaver Configuration File - `weaver.yaml`

## Structure

The following options can be configured in the `weaver.yaml` file:

```yaml
# Specify the configuration of the `text_map` filter.
text_maps:                 # optional
  <text_map_name_1>:
    <input_text>: <output_text>
    # ...
  <text_map_name_2>:
    <input_text>: <output_text>
    # ...

# Specify the configuration of the Jinja template syntax and control whitespace behavior.
# Note: The default syntax is strongly recommended.
template_syntax:           # optional
  block_start: <string>    # default: "{%"
  block_end: <string>      # default: "%}"
  variable_start: <string> # default: "{{"
  variable_end: <string>   # default: "}}"
  comment_start: <string>  # default: "{#"
  comment_end: <string>    # default: "#}"

# Specify the whitespace behavior of the Jinja template engine.
# For more info, see: https://docs.rs/minijinja/latest/minijinja/syntax/index.html#whitespace-control
whitespace_control:
  trim_blocks: <bool>           # default: false
  lstrip_blocks: <bool>         # default: false
  keep_trailing_newline: <bool> # default: false

# Specify a list of acronyms that will be interpreted by the acronym filter. 
acronyms:                  # optional
  - <string>
  - <string>
  - ...

# Specify the configuration of the comment formats.
comment_formats:           # optional
  <format-name>:
    format: markdown|html

    # The following fields are enabled for both markdown and html formats
    # All these fields are optional.
    header: <string>                  # The comment header line (e.g., `/**`)
    prefix: <string>                  # The comment line prefix (e.g., ` * `)
    footer: <string>                  # The comment line footer (e.g., ` */`)
    indent_type: space|tab            # The type of indentation (default: space)
    trim: <bool>                      # Flag to trim the comment content (default: true). 
    remove_trailing_dots: <bool>      # Flag to remove trailing dots from the comment content (default: false).
    enforce_trailing_dots: <bool>     # Flag to enforce trailing dots for the comment content (default: false).

    # The following fields are enabled only when format is set to 'markdown'
    escape_backslashes: <bool>            # Whether to escape backslashes in the markdown (default: false).
    escape_square_brackets: <bool>        # Whether to escape square brackets in markdown (default: false).
    shortcut_reference_links: <bool>      # Use this to convert inlined links into shortcut reference links, similar to those in Go documentation (default: false).
    indent_first_level_list_items: <bool> # Whether to indent the first level of list items in the markdown (default: false).
    default_block_code_language: <string> # The default language for block code snippets (default: "").
    
    # The following fields are enabled only when format is set to 'html'
    old_style_paragraph: <bool>       # Use old-style HTML paragraphs, i.e. single <p> tag (default: false)
    omit_closing_li: <bool>           # Omit closing </li> tags in lists (default: false)
    inline_code_snippet: <jinja-expr> # Jinja expression to render inline code (default: "<c>{{code}}</c>").
    block_code_snippet: <jinja-expr>  # Jinja expression to render block code (default: "<pre>\n{{code}}\n</pre>").
  <other-format-name>:
    # ...
default_comment_format: <format-name>

# Specify the configuration of parameters.
params:                    # optional
  <param_1>: <any_simple_type>
  <param_2>: <any_simple_type>
  # ...

# Each template mapping specifies a jaq filter (compatible with jq)
# to apply to every file matching the template pattern. The application_mode specifies
# how the template should be applied. The application_mode can be `each` or
# `single`. The `each` mode will evaluate the template for each object selected
# by the jaq filter. The `single` mode will evaluate the template once with all
# the objects selected by the jq filter.
#
# Note: jaq is a Rust reimplementation of jq. Most of the jq filters are
# supported. For more information, see https://github.com/01mf02/jaq
templates:
  - template: <file_path or glob>
    filter: <jq_filter>
    application_mode: single|each
    params:                          # optional
      <param_1>: <any_simple_type>
      <param_2>: <any_simple_type>
      # ...
    file_name: <relative_file_path>  # optional
  - ...
```

Note: Both `remove_trailing_dots` and `enforce_trailing_dots` cannot be set to `true` at the same time.

Below a concrete example of a `weaver.yaml` file that could be used to generate Java code
from a semconv specification (incomplete):

```yaml
# This `text_maps` section specifies 2 mappings:
# - `java_types` maps the semconv types to Java types.
# - `java_keys` maps the semconv keys to Java keys.
text_maps:
  java_types:
    int: int
    double: double
    boolean: boolean
    string: String
  java_keys:
    int: intKey
    double: doubleKey
    boolean: booleanKey
    string: stringKey

# A whitespace control configuration compatible with the PHP whitespace control behavior (recommended).
whitespace_control:
  trim_blocks: true
  lstrip_blocks: true

# A list of acronyms that will be interpreted by the acronym filter.
acronyms: ["iOS", "HTTP", "API", "SDK", "CLI", "URL", "JSON", "XML", "HTML"]

# An example of advanced configuration for the comment formats supporting both
# - JavaDoc comment in front of method, function, attribute, and class.
# - and Java end of line comments.
comment_formats:
  javadoc:
    format: html
    header: "/**"
    prefix: " * "
    footer: " */"
    old_style_paragraph: true
    omit_closing_li: true
    inline_code_snippet: "{@code {{code}}}"
    block_code_snippet: "<pre>{@code {{code}}}</pre>"
    trim: true
    remove_trailing_dots: true
  java:
    format: html
    prefix: "// "
    old_style_paragraph: true
    omit_closing_li: true
    inline_code_snippet: "{@code {{code}}}"
    block_code_snippet: "<pre>{@code {{code}}}</pre>"
    trim: true
    remove_trailing_dots: true

templates:
  - template: "attributes.java.j2"
    filter: semconv_grouped_attributes
    application_mode: single

  - template: "metrics.java.j2"
    filter: semconv_grouped_metrics
    application_mode: single
```
 
> [!IMPORTANT]
> **Backward compatibility note**: The field `pattern` has been renamed to `template` in the
> `templates` section. The `pattern` field is still supported for backward compatibility.

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

## Parameters

Weaver supports the definition of parameters both in the `weaver.yaml` file and in the CLI.
The parameters defined in the `weaver.yaml` file will be overridden by the parameters
defined in the CLI.

The parameters defined in the `weaver.yaml` file are accessible in both the JQ expressions
and the Jinja templates (see this [documentation](/crates/weaver_forge/README.md) for more
details). Inside the `weaver.yaml` file, parameters can be defined at multiple levels:
- At the file level, using the top-level `params` section.
- At the template level, using the `params` section inside the `templates` section.

Template-level parameters override file-level parameters. CLI parameters override both
file-level and template-level parameters.

## Example

Imagine we have two slightly different variants for generating the semconv Python code.

```text
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