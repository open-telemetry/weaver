# Command-Line Help for `weaver`

This document contains the help content for the `weaver` command-line program.

**Command Overview:**

* [`weaver`↴](#weaver)
* [`weaver registry`↴](#weaver-registry)
* [`weaver registry check`↴](#weaver-registry-check)
* [`weaver registry generate`↴](#weaver-registry-generate)
* [`weaver registry resolve`↴](#weaver-registry-resolve)
* [`weaver registry search`↴](#weaver-registry-search)
* [`weaver registry stats`↴](#weaver-registry-stats)
* [`weaver registry update-markdown`↴](#weaver-registry-update-markdown)
* [`weaver registry json-schema`↴](#weaver-registry-json-schema)
* [`weaver registry diff`↴](#weaver-registry-diff)
* [`weaver registry emit`↴](#weaver-registry-emit)
* [`weaver registry live-check`↴](#weaver-registry-live-check)
* [`weaver registry mcp`↴](#weaver-registry-mcp)
* [`weaver diagnostic`↴](#weaver-diagnostic)
* [`weaver diagnostic init`↴](#weaver-diagnostic-init)
* [`weaver completion`↴](#weaver-completion)
* [`weaver serve`↴](#weaver-serve)

## `weaver`

Manage semantic convention registry and telemetry schema workflows (OpenTelemetry Project)

**Usage:** `weaver [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `registry` — Manage Semantic Convention Registry
* `diagnostic` — Manage Diagnostic Messages
* `completion` — Generate shell completions
* `serve` — Start the API server (Experimental)

###### **Options:**

* `--debug` — Turn debugging information on. Use twice (--debug --debug) for trace-level logs
* `--quiet` — Turn the quiet mode on (i.e., minimal output)
* `--future` — Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry. Note: `semantic_conventions` main branch should always enable this flag



## `weaver registry`

Manage Semantic Convention Registry

**Usage:** `weaver registry <COMMAND>`

###### **Subcommands:**

* `check` — Validates a semantic convention registry.
* `generate` — Generates artifacts from a semantic convention registry.
* `resolve` — Resolves a semantic convention registry.
* `search` — DEPRECATED - Searches a registry. This command is deprecated and will be removed in a future version. It is not compatible with V2 schema. Please search the generated documentation instead
* `stats` — Calculate a set of general statistics on a semantic convention registry
* `update-markdown` — Update markdown files that contain markers indicating the templates used to update the specified sections
* `json-schema` — Generate the JSON Schema of the resolved registry documents consumed by the template generator and the policy engine.
* `diff` — Generate a diff between two versions of a semantic convention registry.
* `emit` — Emits a semantic convention registry as example signals to your OTLP receiver.
* `live-check` — Perform a live check on sample telemetry by comparing it to a semantic convention registry.
* `mcp` — Run an MCP (Model Context Protocol) server for the semantic convention registry.



## `weaver registry check`

Validates a semantic convention registry.

The validation process for a semantic convention registry involves several steps:
- Loading the semantic convention specifications from a local directory or a git repository.
- Parsing the loaded semantic convention specifications.
- Resolving references and extends clauses within the specifications.
- Checking compliance with specified Rego policies, if provided.

Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.

The process exits with a code of 0 if the registry validation is successful.

**Usage:** `weaver registry check [OPTIONS]`

###### **Options:**

* `-r`, `--registry <REGISTRY>` — Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

  Default value: `https://github.com/open-telemetry/semantic-conventions.git[model]`
* `-s`, `--follow-symlinks` — Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
* `--include-unreferenced` — Boolean flag to include signals and attributes defined in dependency registries, even if they are not explicitly referenced in the current (custom) registry
* `--v2` — Whether or not to output version 2 of the schema. Note: this will impact both output to templates *and* policies

  Default value: `false`
* `--baseline-registry <BASELINE_REGISTRY>` — Parameters to specify the baseline semantic convention registry
* `-p`, `--policy <POLICIES>` — Optional list of policy files or directories to check against the files of the semantic convention registry.  If a directory is provided all `.rego` files in the directory will be loaded
* `--skip-policies` — Skip the policy checks

  Default value: `false`
* `--display-policy-coverage` — Display the policy coverage report (useful for debugging)

  Default value: `false`
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr



## `weaver registry generate`

Generates artifacts from a semantic convention registry.

Rego policies present in the registry or specified using -p or --policy will be automatically validated by the policy engine before the artifact generation phase.

Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.

The process exits with a code of 0 if the generation is successful.

**Usage:** `weaver registry generate [OPTIONS] [TARGET] [OUTPUT]`

###### **Arguments:**

* `<TARGET>` — Target to generate the artifacts for

  Default value: ``
* `<OUTPUT>` — Path to the directory where the generated artifacts will be saved. Default is the `output` directory

  Default value: `output`

###### **Options:**

* `-t`, `--templates <TEMPLATES>` — Path to the directory where the templates are located. Default is the `templates` directory

  Default value: `templates`
* `-c`, `--config <CONFIG>` — List of `weaver.yaml` configuration files to use. When there is a conflict, the last one will override the previous ones for the keys that are defined in both
* `-D`, `--param <PARAM>` — Parameters key=value, defined in the command line, to pass to the templates. The value must be a valid YAML value
* `--params <PARAMS>` — Parameters, defined in a YAML file, to pass to the templates
* `-r`, `--registry <REGISTRY>` — Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

  Default value: `https://github.com/open-telemetry/semantic-conventions.git[model]`
* `-s`, `--follow-symlinks` — Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
* `--include-unreferenced` — Boolean flag to include signals and attributes defined in dependency registries, even if they are not explicitly referenced in the current (custom) registry
* `--v2` — Whether or not to output version 2 of the schema. Note: this will impact both output to templates *and* policies

  Default value: `false`
* `-p`, `--policy <POLICIES>` — Optional list of policy files or directories to check against the files of the semantic convention registry.  If a directory is provided all `.rego` files in the directory will be loaded
* `--skip-policies` — Skip the policy checks

  Default value: `false`
* `--display-policy-coverage` — Display the policy coverage report (useful for debugging)

  Default value: `false`
* `--future` — Enable the most recent validation rules for the semconv registry. It is recommended to enable this flag when checking a new registry

  Default value: `false`
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr



## `weaver registry resolve`

Resolves a semantic convention registry.

Rego policies present in the registry or specified using -p or --policy will be automatically validated by the policy engine before the artifact generation phase.

Note: The `-d` and `--registry-git-sub-dir` options are only used when the registry is a Git URL otherwise these options are ignored.

The process exits with a code of 0 if the resolution is successful.

**Usage:** `weaver registry resolve [OPTIONS]`

###### **Options:**

* `-r`, `--registry <REGISTRY>` — Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

  Default value: `https://github.com/open-telemetry/semantic-conventions.git[model]`
* `-s`, `--follow-symlinks` — Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
* `--include-unreferenced` — Boolean flag to include signals and attributes defined in dependency registries, even if they are not explicitly referenced in the current (custom) registry
* `--v2` — Whether or not to output version 2 of the schema. Note: this will impact both output to templates *and* policies

  Default value: `false`
* `--lineage` — Flag to indicate if lineage information should be included in the resolved schema (not yet implemented)

  Default value: `false`
* `-o`, `--output <OUTPUT>` — Output file to write the resolved schema to If not specified, the resolved schema is printed to stdout
* `-f`, `--format <FORMAT>` — Output format for the resolved schema If not specified, the resolved schema is printed in YAML format Supported formats: yaml, json Default format: yaml Example: `--format json`

  Default value: `yaml`

  Possible values:
  - `yaml`:
    YAML format
  - `json`:
    JSON format

* `-p`, `--policy <POLICIES>` — Optional list of policy files or directories to check against the files of the semantic convention registry.  If a directory is provided all `.rego` files in the directory will be loaded
* `--skip-policies` — Skip the policy checks

  Default value: `false`
* `--display-policy-coverage` — Display the policy coverage report (useful for debugging)

  Default value: `false`
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr



## `weaver registry search`

DEPRECATED - Searches a registry. This command is deprecated and will be removed in a future version. It is not compatible with V2 schema. Please search the generated documentation instead

**Usage:** `weaver registry search [OPTIONS] [SEARCH_STRING]`

###### **Arguments:**

* `<SEARCH_STRING>` — An (optional) search string to use.  If specified, will return matching values on the command line. Otherwise, runs an interactive terminal UI

###### **Options:**

* `-r`, `--registry <REGISTRY>` — Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

  Default value: `https://github.com/open-telemetry/semantic-conventions.git[model]`
* `-s`, `--follow-symlinks` — Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
* `--include-unreferenced` — Boolean flag to include signals and attributes defined in dependency registries, even if they are not explicitly referenced in the current (custom) registry
* `--v2` — Whether or not to output version 2 of the schema. Note: this will impact both output to templates *and* policies

  Default value: `false`
* `--lineage` — Flag to indicate if lineage information should be included in the resolved schema (not yet implemented)

  Default value: `false`
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr



## `weaver registry stats`

Calculate a set of general statistics on a semantic convention registry

**Usage:** `weaver registry stats [OPTIONS]`

###### **Options:**

* `-r`, `--registry <REGISTRY>` — Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

  Default value: `https://github.com/open-telemetry/semantic-conventions.git[model]`
* `-s`, `--follow-symlinks` — Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
* `--include-unreferenced` — Boolean flag to include signals and attributes defined in dependency registries, even if they are not explicitly referenced in the current (custom) registry
* `--v2` — Whether or not to output version 2 of the schema. Note: this will impact both output to templates *and* policies

  Default value: `false`
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr



## `weaver registry update-markdown`

Update markdown files that contain markers indicating the templates used to update the specified sections

**Usage:** `weaver registry update-markdown [OPTIONS] --target <TARGET> <MARKDOWN_DIR>`

###### **Arguments:**

* `<MARKDOWN_DIR>` — Path to the directory where the markdown files are located

###### **Options:**

* `-r`, `--registry <REGISTRY>` — Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

  Default value: `https://github.com/open-telemetry/semantic-conventions.git[model]`
* `-s`, `--follow-symlinks` — Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
* `--include-unreferenced` — Boolean flag to include signals and attributes defined in dependency registries, even if they are not explicitly referenced in the current (custom) registry
* `--v2` — Whether or not to output version 2 of the schema. Note: this will impact both output to templates *and* policies

  Default value: `false`
* `--dry-run` — Whether or not to run updates in dry-run mode

  Default value: `false`
* `--attribute-registry-base-url <ATTRIBUTE_REGISTRY_BASE_URL>` — Optional path to the attribute registry. If provided, all attributes will be linked here
* `-D`, `--param <PARAM>` — Parameters key=value, defined in the command line, to pass to the templates. The value must be a valid YAML value
* `--params <PARAMS>` — Parameters, defined in a YAML file, to pass to the templates
* `-t`, `--templates <TEMPLATES>` — Path to the directory where the templates are located. Default is the `templates` directory. Note: `registry update-markdown` will look for a specific jinja template: {templates}/{target}/snippet.md.j2

  Default value: `templates`
* `--target <TARGET>` — If provided, the target to generate snippets with. Note: `registry update-markdown` will look for a specific jinja template: {templates}/{target}/snippet.md.j2
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr



## `weaver registry json-schema`

Generate the JSON Schema of the resolved registry documents consumed by the template generator and the policy engine.

The produced JSON Schema can be used to generate documentation of the resolved registry format or to generate code in your language of choice if you need to interact with the resolved registry format for any reason.

**Usage:** `weaver registry json-schema [OPTIONS]`

###### **Options:**

* `-j`, `--json-schema <JSON_SCHEMA>` — The type of JSON schema to generate

  Default value: `resolved-registry`

  Possible values:
  - `resolved-registry`:
    The JSON schema of a resolved registry
  - `semconv-group`:
    The JSON schema of a semantic convention group
  - `semconv-definition-v2`:
    The JSON schema of the V2 definition
  - `resolved-registry-v2`:
    The JSON schema of the V2 resolved registry
  - `forge-registry-v2`:
    The JSON schema we send to Rego / Jinja
  - `diff`:
    The JSON schema of the diff
  - `diff-v2`:
    The JSON schema of the diff V2

* `-o`, `--output <OUTPUT>` — Output file to write the JSON schema to If not specified, the JSON schema is printed to stdout
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr



## `weaver registry diff`

Generate a diff between two versions of a semantic convention registry.

This diff can then be rendered in multiple formats:
- a console-friendly format (default: ansi),
- a structured document in JSON format,
- ...

**Usage:** `weaver registry diff [OPTIONS] --baseline-registry <BASELINE_REGISTRY>`

###### **Options:**

* `-r`, `--registry <REGISTRY>` — Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

  Default value: `https://github.com/open-telemetry/semantic-conventions.git[model]`
* `-s`, `--follow-symlinks` — Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
* `--include-unreferenced` — Boolean flag to include signals and attributes defined in dependency registries, even if they are not explicitly referenced in the current (custom) registry
* `--v2` — Whether or not to output version 2 of the schema. Note: this will impact both output to templates *and* policies

  Default value: `false`
* `--baseline-registry <BASELINE_REGISTRY>` — Parameters to specify the baseline semantic convention registry
* `--diff-format <DIFF_FORMAT>` — Format used to render the schema changes. Predefined formats are: ansi, json, and markdown

  Default value: `ansi`
* `--diff-template <DIFF_TEMPLATE>` — Path to the directory where the schema changes templates are located

  Default value: `diff_templates`
* `-o`, `--output <OUTPUT>` — Path to the directory where the generated artifacts will be saved. If not specified, the diff report is printed to stdout
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr



## `weaver registry emit`

Emits a semantic convention registry as example signals to your OTLP receiver.

This uses the standard OpenTelemetry SDK, defaulting to OTLP gRPC on localhost:4317.

**Usage:** `weaver registry emit [OPTIONS]`

###### **Options:**

* `-r`, `--registry <REGISTRY>` — Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

  Default value: `https://github.com/open-telemetry/semantic-conventions.git[model]`
* `-s`, `--follow-symlinks` — Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
* `--include-unreferenced` — Boolean flag to include signals and attributes defined in dependency registries, even if they are not explicitly referenced in the current (custom) registry
* `--v2` — Whether or not to output version 2 of the schema. Note: this will impact both output to templates *and* policies

  Default value: `false`
* `-p`, `--policy <POLICIES>` — Optional list of policy files or directories to check against the files of the semantic convention registry.  If a directory is provided all `.rego` files in the directory will be loaded
* `--skip-policies` — Skip the policy checks

  Default value: `false`
* `--display-policy-coverage` — Display the policy coverage report (useful for debugging)

  Default value: `false`
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr
* `--stdout` — Write the telemetry to standard output
* `--endpoint <ENDPOINT>` — Endpoint for the OTLP receiver. OTEL_EXPORTER_OTLP_ENDPOINT env var will override this

  Default value: `http://localhost:4317`



## `weaver registry live-check`

Perform a live check on sample telemetry by comparing it to a semantic convention registry.

Includes: Flexible input ingestion, configurable assessment, and template-based output.

**Usage:** `weaver registry live-check [OPTIONS]`

###### **Options:**

* `-r`, `--registry <REGISTRY>` — Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

  Default value: `https://github.com/open-telemetry/semantic-conventions.git[model]`
* `-s`, `--follow-symlinks` — Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
* `--include-unreferenced` — Boolean flag to include signals and attributes defined in dependency registries, even if they are not explicitly referenced in the current (custom) registry
* `--v2` — Whether or not to output version 2 of the schema. Note: this will impact both output to templates *and* policies

  Default value: `false`
* `-p`, `--policy <POLICIES>` — Optional list of policy files or directories to check against the files of the semantic convention registry.  If a directory is provided all `.rego` files in the directory will be loaded
* `--skip-policies` — Skip the policy checks

  Default value: `false`
* `--display-policy-coverage` — Display the policy coverage report (useful for debugging)

  Default value: `false`
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr
* `--input-source <INPUT_SOURCE>` — Where to read the input telemetry from. {file path} | stdin | otlp

  Default value: `otlp`
* `--input-format <INPUT_FORMAT>` — The format of the input telemetry. (Not required for OTLP). text | json

  Default value: `json`
* `--format <FORMAT>` — Format used to render the report. Predefined formats are: ansi, json

  Default value: `ansi`
* `--templates <TEMPLATES>` — Path to the directory where the templates are located

  Default value: `live_check_templates`
* `--no-stream` — Disable stream mode. Use this flag to disable streaming output.

   When the output is STDOUT, Ingesters that support streaming (STDIN and OTLP), by default output the live check results for each entity as they are ingested.

  Default value: `false`
* `--no-stats` — Disable statistics accumulation. This is useful for long-running live check sessions. Typically combined with --emit-otlp-logs and --output=none

  Default value: `false`
* `-o`, `--output <OUTPUT>` — Path to the directory where the generated artifacts will be saved. If not specified, the report is printed to stdout. Use "none" to disable all template output rendering (useful when emitting OTLP logs)
* `--otlp-grpc-address <OTLP_GRPC_ADDRESS>` — Address used by the gRPC OTLP listener

  Default value: `0.0.0.0`
* `--otlp-grpc-port <OTLP_GRPC_PORT>` — Port used by the gRPC OTLP listener

  Default value: `4317`
* `--emit-otlp-logs` — Enable OTLP log emission for live check policy findings

  Default value: `false`
* `--otlp-logs-endpoint <OTLP_LOGS_ENDPOINT>` — OTLP endpoint for log emission

  Default value: `http://localhost:4317`
* `--otlp-logs-stdout` — Use stdout for OTLP log emission (debug mode)

  Default value: `false`
* `--admin-port <ADMIN_PORT>` — Port used by the HTTP admin port (endpoints: /stop)

  Default value: `4320`
* `--inactivity-timeout <INACTIVITY_TIMEOUT>` — Max inactivity time in seconds before stopping the listener

  Default value: `10`
* `--advice-policies <ADVICE_POLICIES>` — Advice policies directory. Set this to override the default policies
* `--advice-preprocessor <ADVICE_PREPROCESSOR>` — Advice preprocessor. A jq script to preprocess the registry data before passing to rego.

   Rego policies are run for each sample as it arrives in a stream. The preprocessor can be used to create a new data structure that is more efficient for the rego policies versus processing the data for every sample.



## `weaver registry mcp`

Run an MCP (Model Context Protocol) server for the semantic convention registry.

This server exposes the registry to LLMs, enabling natural language
queries for finding and understanding semantic conventions while writing
instrumentation code.

The server communicates over stdio using JSON-RPC.

**Usage:** `weaver registry mcp [OPTIONS]`

###### **Options:**

* `-r`, `--registry <REGISTRY>` — Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

  Default value: `https://github.com/open-telemetry/semantic-conventions.git[model]`
* `-s`, `--follow-symlinks` — Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
* `--include-unreferenced` — Boolean flag to include signals and attributes defined in dependency registries, even if they are not explicitly referenced in the current (custom) registry
* `--v2` — Whether or not to output version 2 of the schema. Note: this will impact both output to templates *and* policies

  Default value: `false`
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr
* `--advice-policies <ADVICE_POLICIES>` — Advice policies directory. Set this to override the default policies
* `--advice-preprocessor <ADVICE_PREPROCESSOR>` — Advice preprocessor. A jq script to preprocess the registry data before passing to rego.

   Rego policies are run for each sample as it arrives. The preprocessor can be used to create a new data structure that is more efficient for the rego policies versus processing the data for every sample.



## `weaver diagnostic`

Manage Diagnostic Messages

**Usage:** `weaver diagnostic <COMMAND>`

###### **Subcommands:**

* `init` — Initializes a `diagnostic_templates` directory to define or override diagnostic output formats



## `weaver diagnostic init`

Initializes a `diagnostic_templates` directory to define or override diagnostic output formats

**Usage:** `weaver diagnostic init [OPTIONS] [TARGET]`

###### **Arguments:**

* `<TARGET>` — Optional target to initialize the diagnostic templates for. If empty, all default templates will be extracted

  Default value: ``

###### **Options:**

* `-t`, `--diagnostic-templates-dir <DIAGNOSTIC_TEMPLATES_DIR>` — Optional path where the diagnostic templates directory should be created

  Default value: `diagnostic_templates`
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr



## `weaver completion`

Generate shell completions

**Usage:** `weaver completion <SHELL>`

###### **Arguments:**

* `<SHELL>` — The shell to generate the completions for

  Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`




## `weaver serve`

Start the API server (Experimental)

**Usage:** `weaver serve [OPTIONS]`

###### **Options:**

* `-r`, `--registry <REGISTRY>` — Local folder, Git repo URL, or Git archive URL of the semantic convention registry. For Git URLs, a sub-folder can be specified using the `[sub-folder]` syntax after the URL

  Default value: `https://github.com/open-telemetry/semantic-conventions.git[model]`
* `-s`, `--follow-symlinks` — Boolean flag to specify whether to follow symlinks when loading the registry. Default is false
* `--include-unreferenced` — Boolean flag to include signals and attributes defined in dependency registries, even if they are not explicitly referenced in the current (custom) registry
* `--v2` — Whether or not to output version 2 of the schema. Note: this will impact both output to templates *and* policies

  Default value: `false`
* `-p`, `--policy <POLICIES>` — Optional list of policy files or directories to check against the files of the semantic convention registry.  If a directory is provided all `.rego` files in the directory will be loaded
* `--skip-policies` — Skip the policy checks

  Default value: `false`
* `--display-policy-coverage` — Display the policy coverage report (useful for debugging)

  Default value: `false`
* `--bind <BIND>` — Address to bind the server to

  Default value: `127.0.0.1:8080`
* `--cors-origins <CORS_ORIGINS>` — Allowed CORS origins (comma-separated). Use '*' for any origin. If not specified, CORS is disabled (same-origin only)
* `--diagnostic-format <DIAGNOSTIC_FORMAT>` — Format used to render the diagnostic messages. Predefined formats are: ansi, json, gh_workflow_command

  Default value: `ansi`
* `--diagnostic-template <DIAGNOSTIC_TEMPLATE>` — Path to the directory where the diagnostic templates are located

  Default value: `diagnostic_templates`
* `--diagnostic-stdout` — Send the output to stdout instead of stderr



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

