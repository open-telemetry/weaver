# Usage

```
Manage semantic convention registry and telemetry schema workflows (OpenTelemetry Project)

Usage: weaver [OPTIONS] [COMMAND]

Commands:
  registry  Manage Semantic Convention Registry
  help      Print this message or the help of the given subcommand(s)

Options:
  -d, --debug...  Turn debugging information on
  -q, --quiet     Turn the quiet mode on (i.e., minimal output)
  -h, --help      Print help
  -V, --version   Print version
```

## registry check

```
Validates a semantic convention registry.

The validation process for a semantic convention registry involves several steps:
- Loading the semantic convention specifications from a local directory or a git repository.
- Parsing the loaded semantic convention specifications.
- Resolving references and extends clauses within the specifications.
- Checking compliance with specified Rego policies, if provided.

The process exits with a code of 0 if the registry validation is successful.

Usage: weaver registry check [OPTIONS]

Options:
  -r, --registry <REGISTRY>
          Local path or Git URL of the semantic convention registry to check

          [default: https://github.com/open-telemetry/semantic-conventions.git]

  -p, --policy <POLICIES>
          Optional list of policy files to check against the files of the semantic convention registry

      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

          [default: ansi]

      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located

          [default: diagnostic_templates]

  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false

  -h, --help
          Print help (see a summary with '-h')
```

## registry generate

```
Generates artifacts from a semantic convention registry.

Rego policies present in the registry or specified using -p or --policy will be automatically validated by the policy engine before the artifact generation phase.

The process exits with a code of 0 if the generation is successful.

Usage: weaver registry generate [OPTIONS] <TARGET> [OUTPUT]

Arguments:
  <TARGET>
          Target to generate the artifacts for

  [OUTPUT]
          Path to the directory where the generated artifacts will be saved. Default is the `output` directory

          [default: output]

Options:
  -t, --templates <TEMPLATES>
          Path to the directory where the templates are located. Default is the `templates` directory

          [default: templates]

  -r, --registry <REGISTRY>
          Local path or Git URL of the semantic convention registry

          [default: https://github.com/open-telemetry/semantic-conventions.git]

  -p, --policy <POLICIES>
          Optional list of policy files to check against the files of the semantic convention registry

      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

          [default: ansi]

      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located

          [default: diagnostic_templates]

  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false

  -h, --help
          Print help (see a summary with '-h')
  
```

## registry resolve

```
Resolves a semantic convention registry.

Rego policies present in the registry or specified using -p or --policy will be automatically validated by the policy engine before the artifact generation phase.

The process exits with a code of 0 if the resolution is successful.

Usage: weaver registry resolve [OPTIONS]

Options:
  -r, --registry <REGISTRY>
          Local path or Git URL of the semantic convention registry

          [default: https://github.com/open-telemetry/semantic-conventions.git]

      --catalog
          Flag to indicate if the shared catalog should be included in the resolved schema

      --lineage
          Flag to indicate if lineage information should be included in the resolved schema (not yet implemented)

  -o, --output <OUTPUT>
          Output file to write the resolved schema to If not specified, the resolved schema is printed to stdout

  -f, --format <FORMAT>
          Output format for the resolved schema If not specified, the resolved schema is printed in YAML format Supported formats: yaml, json Default format: yaml Example: `--format json`

          [default: yaml]

          Possible values:
          - yaml: YAML format
          - json: JSON format

  -p, --policy <POLICIES>
          Optional list of policy files to check against the files of the semantic convention registry

      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

          [default: ansi]

      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located

          [default: diagnostic_templates]

  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false

  -h, --help
          Print help (see a summary with '-h')
```

## registry diff

```
Generate a diff between two versions of a semantic convention registry.

This diff can then be rendered in multiple formats:
- a console-friendly format (default: ansi),
- a structured document in JSON or YAML format,
- ...

Usage: weaver registry diff [OPTIONS] --baseline-registry <BASELINE_REGISTRY> [OUTPUT]

Arguments:
  [OUTPUT]
          Path to the directory where the generated artifacts will be saved. Default is the `output` directory

          [default: output]

Options:
      --debug...
          Turn debugging information on

  -r, --registry <REGISTRY>
          Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

          [default: https://github.com/open-telemetry/semantic-conventions.git[model]]

      --baseline-registry <BASELINE_REGISTRY>
          Parameters to specify the baseline semantic convention registry

      --quiet
          Turn the quiet mode on (i.e., minimal output)

      --diff-format <DIFF_FORMAT>
          Format used to render the schema changes. Predefined formats are: ansi, json, yaml, and markdown

          [default: ansi]

      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag

      --diff-template <DIFF_TEMPLATE>
          Path to the directory where the schema changes templates are located

          [default: diff_templates]

      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

          [default: ansi]

      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located

          [default: diagnostic_templates]

  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false

  -h, --help
          Print help (see a summary with '-h')
```

## registry update-markdown

```
Update markdown files that contain markers indicating the templates used to update the specified sections

Usage: weaver registry update-markdown [OPTIONS] <MARKDOWN_DIR>

Arguments:
  <MARKDOWN_DIR>  Path to the directory where the markdown files are located

Options:
  -r, --registry <REGISTRY>
          Local path or Git URL of the semantic convention registry to check [default: https://github.com/open-telemetry/semantic-conventions.git]
      --dry-run
          Whether or not to run updates in dry-run mode
      --attribute-registry-base-url <ATTRIBUTE_REGISTRY_BASE_URL>
          Optional path to the attribute registry. If provided, all attributes will be linked here
  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
  -h, --help
          Print help
```

## registry stats

```
Calculate and display a set of general statistics on a registry (not yet implemented)

Usage: weaver registry stats [OPTIONS]

Options:
  -r, --registry <REGISTRY>
          Local path or Git URL of the semantic convention registry [default: https://github.com/open-telemetry/semantic-conventions.git]
  -h, --help
          Print help
```

## diagnostic init

```
Initializes a `diagnostic_templates` directory to define or override diagnostic output formats

Usage: weaver diagnostic init [OPTIONS] [TARGET]

Arguments:
  [TARGET]  Optional target to initialize the diagnostic templates for. If empty, all default templates will be extracted [default: ]

Options:
  -t, --diagnostic-templates-dir <DIAGNOSTIC_TEMPLATES_DIR>
          Optional path where the diagnostic templates directory should be created [default: diagnostic_templates]
      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command [default: ansi]
      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located [default: diagnostic_templates]
  -h, --help
          Print help
```
