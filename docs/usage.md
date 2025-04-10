# Usage

```text
Manage semantic convention registry and telemetry schema workflows (OpenTelemetry Project)

Usage: weaver [OPTIONS] <COMMAND>

Commands:
  registry    Manage Semantic Convention Registry
  diagnostic  Manage Diagnostic Messages
  help        Print this message or the help of the given subcommand(s)

Options:
      --debug...  Turn debugging information on
      --quiet     Turn the quiet mode on (i.e., minimal output)
      --future    Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag
  -h, --help      Print help
  -V, --version   Print version
```

## registry

```text
Manage Semantic Convention Registry

Usage: weaver registry [OPTIONS] <COMMAND>

Commands:
  check            Validates a semantic convention registry.
  generate         Generates artifacts from a semantic convention registry.
  resolve          Resolves a semantic convention registry.
  search           Searches a registry (Note: Experimental and subject to change)
  stats            Calculate a set of general statistics on a semantic convention registry
  update-markdown  Update markdown files that contain markers indicating the templates used to update the specified sections
  json-schema      Generate the JSON Schema of the resolved registry documents consumed by the template generator and the policy engine.
  diff             Generate a diff between two versions of a semantic convention registry.
  live-check       Check the conformance level of an OTLP stream against a semantic convention registry.
  emit             Emits a semantic convention registry as example signals to your OTLP receiver.
  help             Print this message or the help of the given subcommand(s)

Options:
      --debug...  Turn debugging information on
      --quiet     Turn the quiet mode on (i.e., minimal output)
      --future    Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag
  -h, --help      Print help
```

### registry check

```text
Validates a semantic convention registry.

The validation process for a semantic convention registry involves several steps:
- Loading the semantic convention specifications from a local directory or a git repository.
- Parsing the loaded semantic convention specifications.
- Resolving references and extends clauses within the specifications.
- Checking compliance with specified Rego policies, if provided.

Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.

The process exits with a code of 0 if the registry validation is successful.

Usage: weaver registry check [OPTIONS]

Options:
      --debug...
          Turn debugging information on

  -r, --registry <REGISTRY>
          Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

          [default: https://github.com/open-telemetry/semantic-conventions.git[model]]

      --quiet
          Turn the quiet mode on (i.e., minimal output)

  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false

      --baseline-registry <BASELINE_REGISTRY>
          Parameters to specify the baseline semantic convention registry

      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag

  -p, --policy <POLICIES>
          Optional list of policy files or directories to check against the files of the semantic convention registry.  If a directory is provided all `.rego` files in the directory will be loaded

      --skip-policies
          Skip the policy checks

      --display-policy-coverage
          Display the policy coverage report (useful for debugging)

      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

          [default: ansi]

      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located

          [default: diagnostic_templates]

  -h, --help
          Print help (see a summary with '-h')
```

### registry generate

```text
Generates artifacts from a semantic convention registry.

Rego policies present in the registry or specified using -p or --policy will be automatically validated by the policy engine before the artifact generation phase.

Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.

The process exits with a code of 0 if the generation is successful.

Usage: weaver registry generate [OPTIONS] <TARGET> [OUTPUT]

Arguments:
  <TARGET>
          Target to generate the artifacts for

  [OUTPUT]
          Path to the directory where the generated artifacts will be saved. Default is the `output` directory

          [default: output]

Options:
      --debug...
          Turn debugging information on

  -t, --templates <TEMPLATES>
          Path to the directory where the templates are located. Default is the `templates` directory

          [default: templates]

  -c, --config <CONFIG>
          List of `weaver.yaml` configuration files to use. When there is a conflict, the last one will override the previous ones for the keys that are defined in both

      --quiet
          Turn the quiet mode on (i.e., minimal output)

  -D, --param <PARAM>
          Parameters key=value, defined in the command line, to pass to the templates. The value must be a valid YAML value

      --params <PARAMS>
          Parameters, defined in a YAML file, to pass to the templates

  -r, --registry <REGISTRY>
          Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

          [default: https://github.com/open-telemetry/semantic-conventions.git[model]]

  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false

  -p, --policy <POLICIES>
          Optional list of policy files or directories to check against the files of the semantic convention registry.  If a directory is provided all `.rego` files in the directory will be loaded

      --skip-policies
          Skip the policy checks

      --display-policy-coverage
          Display the policy coverage report (useful for debugging)

      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry

      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

          [default: ansi]

      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located

          [default: diagnostic_templates]

  -h, --help
          Print help (see a summary with '-h')
```

### registry resolve

```text
Resolves a semantic convention registry.

Rego policies present in the registry or specified using -p or --policy will be automatically validated by the policy engine before the artifact generation phase.

Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.

The process exits with a code of 0 if the resolution is successful.

Usage: weaver registry resolve [OPTIONS]

Options:
      --debug...
          Turn debugging information on

  -r, --registry <REGISTRY>
          Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

          [default: https://github.com/open-telemetry/semantic-conventions.git[model]]

      --quiet
          Turn the quiet mode on (i.e., minimal output)

  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false

      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag

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
          Optional list of policy files or directories to check against the files of the semantic convention registry.  If a directory is provided all `.rego` files in the directory will be loaded

      --skip-policies
          Skip the policy checks

      --display-policy-coverage
          Display the policy coverage report (useful for debugging)

      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

          [default: ansi]

      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located

          [default: diagnostic_templates]

  -h, --help
          Print help (see a summary with '-h')
```

### registry search

```text
Searches a registry (Note: Experimental and subject to change)

Usage: weaver registry search [OPTIONS] [SEARCH_STRING]

Arguments:
  [SEARCH_STRING]  An (optional) search string to use.  If specified, will return matching values on the command line. Otherwise, runs an interactive terminal UI

Options:
      --debug...
          Turn debugging information on
  -r, --registry <REGISTRY>
          Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL [default: https://github.com/open-telemetry/semantic-conventions.git[model]]
      --quiet
          Turn the quiet mode on (i.e., minimal output)
  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag
      --lineage
          Flag to indicate if lineage information should be included in the resolved schema (not yet implemented)
      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command [default: ansi]
      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located [default: diagnostic_templates]
  -h, --help
          Print help
```

### registry stats

```text
Calculate a set of general statistics on a semantic convention registry

Usage: weaver registry stats [OPTIONS]

Options:
      --debug...
          Turn debugging information on
  -r, --registry <REGISTRY>
          Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL [default: https://github.com/open-telemetry/semantic-conventions.git[model]]
      --quiet
          Turn the quiet mode on (i.e., minimal output)
  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command [default: ansi]
      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag
      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located [default: diagnostic_templates]
  -h, --help
          Print help
```

### registry update-markdown

```text
Update markdown files that contain markers indicating the templates used to update the specified sections

Usage: weaver registry update-markdown [OPTIONS] --target <TARGET> <MARKDOWN_DIR>

Arguments:
  <MARKDOWN_DIR>  Path to the directory where the markdown files are located

Options:
      --debug...
          Turn debugging information on
  -r, --registry <REGISTRY>
          Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL [default: https://github.com/open-telemetry/semantic-conventions.git[model]]
      --quiet
          Turn the quiet mode on (i.e., minimal output)
  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
      --dry-run
          Whether or not to run updates in dry-run mode
      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag
      --attribute-registry-base-url <ATTRIBUTE_REGISTRY_BASE_URL>
          Optional path to the attribute registry. If provided, all attributes will be linked here
  -t, --templates <TEMPLATES>
          Path to the directory where the templates are located. Default is the `templates` directory. Note: `registry update-markdown` will look for a specific jinja template: {templates}/{target}/snippet.md.j2 [default: templates]
      --target <TARGET>
          If provided, the target to generate snippets with. Note: `registry update-markdown` will look for a specific jinja template: {templates}/{target}/snippet.md.j2
      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command [default: ansi]
      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located [default: diagnostic_templates]
  -h, --help
          Print help
```

### registry json-schema

```text
Generate the JSON Schema of the resolved registry documents consumed by the template generator and the policy engine.

The produced JSON Schema can be used to generate documentation of the resolved registry format or to generate code in your language of choice if you need to interact with the resolved registry format for any reason.

Usage: weaver registry json-schema [OPTIONS]

Options:
      --debug...
          Turn debugging information on

  -o, --output <OUTPUT>
          Output file to write the JSON schema to If not specified, the JSON schema is printed to stdout

      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

          [default: ansi]

      --quiet
          Turn the quiet mode on (i.e., minimal output)

      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located

          [default: diagnostic_templates]

      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag

  -h, --help
          Print help (see a summary with '-h')
```

### registry diff

```text
Generate a diff between two versions of a semantic convention registry.

This diff can then be rendered in multiple formats:
- a console-friendly format (default: ansi),
- a structured document in JSON format,
- ...

Usage: weaver registry diff [OPTIONS] --baseline-registry <BASELINE_REGISTRY>

Options:
      --debug...
          Turn debugging information on

  -r, --registry <REGISTRY>
          Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

          [default: https://github.com/open-telemetry/semantic-conventions.git[model]]

      --quiet
          Turn the quiet mode on (i.e., minimal output)

  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false

      --baseline-registry <BASELINE_REGISTRY>
          Parameters to specify the baseline semantic convention registry

      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag

      --diff-format <DIFF_FORMAT>
          Format used to render the schema changes. Predefined formats are: ansi, json, and markdown

          [default: ansi]

      --diff-template <DIFF_TEMPLATE>
          Path to the directory where the schema changes templates are located

          [default: diff_templates]

  -o, --output <OUTPUT>
          Path to the directory where the generated artifacts will be saved. If not specified, the diff report is printed to stdout

      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

          [default: ansi]

      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located

          [default: diagnostic_templates]

  -h, --help
          Print help (see a summary with '-h')
```

### registry live-check

```text
Check the conformance level of an OTLP stream against a semantic convention registry.

This command starts an OTLP listener and compares each received OTLP message with the
registry provided as a parameter. When the command is stopped (see stop conditions),
a conformance/coverage report is generated. The purpose of this command is to be used
in a CI/CD pipeline to validate the telemetry stream from an application or service
against a registry.

The currently supported stop conditions are: CTRL+C (SIGINT), SIGHUP, the HTTP /stop
endpoint, and a maximum duration of no OTLP message reception.

Usage: weaver registry live-check [OPTIONS]

Options:
      --debug...
          Turn debugging information on

  -r, --registry <REGISTRY>
          Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

          [default: https://github.com/open-telemetry/semantic-conventions.git[model]]

      --quiet
          Turn the quiet mode on (i.e., minimal output)

  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false

      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag

      --otlp-grpc-address <OTLP_GRPC_ADDRESS>
          Address used by the gRPC OTLP listener

          [default: 0.0.0.0]

  -p, --otlp-grpc-port <OTLP_GRPC_PORT>
          Port used by the gRPC OTLP listener

          [default: 4317]

  -a, --admin-port <ADMIN_PORT>
          Port used by the HTTP admin port (endpoints: /stop)

          [default: 4320]

  -t, --inactivity-timeout <INACTIVITY_TIMEOUT>
          Max inactivity time in seconds before stopping the listener

          [default: 10]

      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

          [default: ansi]

      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located

          [default: diagnostic_templates]

  -h, --help
          Print help (see a summary with '-h')
```

### registry emit

```text
Emits a semantic convention registry as example signals to your OTLP receiver.

This uses the standard OpenTelemetry SDK, defaulting to OTLP gRPC on localhost:4317.

Usage: weaver registry emit [OPTIONS]

Options:
      --debug...
          Turn debugging information on

  -r, --registry <REGISTRY>
          Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

          [default: https://github.com/open-telemetry/semantic-conventions.git[model]]

      --quiet
          Turn the quiet mode on (i.e., minimal output)

  -s, --follow-symlinks
          Boolean flag to specify whether to follow symlinks when loading the registry. Default is false

      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag

  -p, --policy <POLICIES>
          Optional list of policy files or directories to check against the files of the semantic convention registry.  If a directory is provided all `.rego` files in the directory will be loaded

      --skip-policies
          Skip the policy checks

      --display-policy-coverage
          Display the policy coverage report (useful for debugging)

      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

          [default: ansi]

      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located

          [default: diagnostic_templates]

      --stdout
          Write the telemetry to standard output

      --endpoint <ENDPOINT>
          Endpoint for the OTLP receiver. OTEL_EXPORTER_OTLP_ENDPOINT env var will override this

          [default: http://localhost:4317]

  -h, --help
          Print help (see a summary with '-h')
```

## diagnostic

```text
Manage Diagnostic Messages

Usage: weaver diagnostic [OPTIONS] <COMMAND>

Commands:
  init  Initializes a `diagnostic_templates` directory to define or override diagnostic output formats
  help  Print this message or the help of the given subcommand(s)

Options:
      --debug...  Turn debugging information on
      --quiet     Turn the quiet mode on (i.e., minimal output)
      --future    Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag
  -h, --help      Print help
```

### diagnostic init

```text
Initializes a `diagnostic_templates` directory to define or override diagnostic output formats

Usage: weaver diagnostic init [OPTIONS] [TARGET]

Arguments:
  [TARGET]  Optional target to initialize the diagnostic templates for. If empty, all default templates will be extracted [default: ]

Options:
      --debug...
          Turn debugging information on
  -t, --diagnostic-templates-dir <DIAGNOSTIC_TEMPLATES_DIR>
          Optional path where the diagnostic templates directory should be created [default: diagnostic_templates]
      --diagnostic-format <DIAGNOSTIC_FORMAT>
          Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command [default: ansi]
      --quiet
          Turn the quiet mode on (i.e., minimal output)
      --diagnostic-template <DIAGNOSTIC_TEMPLATE>
          Path to the directory where the diagnostic templates are located [default: diagnostic_templates]
      --future
          Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag
  -h, --help
          Print help
```
