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
Validates a registry (i.e., parsing, resolution of references, extends clauses, and constraints)

Usage: weaver registry check [OPTIONS]

Options:
  -r, --registry <REGISTRY>
          Local path or Git URL of the semantic convention registry to check [default: https://github.com/open-telemetry/semantic-conventions.git]
  -d, --registry-git-sub-dir <REGISTRY_GIT_SUB_DIR>
          Optional path in the Git repository where the semantic convention registry is located [default: model]
  -b, --before-resolution-policies <BEFORE_RESOLUTION_POLICIES>
          Optional list of policy files to check against the files of the semantic convention registry before the resolution process
  -h, --help
          Print help
```

> Note: The `-d` and `--registry-git-sub-dir` options are only used when the
> registry is a Git URL otherwise these options are ignored.

## registry generate

```
Generates artifacts from a registry

Usage: weaver registry generate [OPTIONS] <TARGET> [OUTPUT]

Arguments:
  <TARGET>  Target to generate the artifacts for
  [OUTPUT]  Path to the directory where the generated artifacts will be saved. Default is the `output` directory [default: output]

Options:
  -t, --templates <TEMPLATES>
          Path to the directory where the templates are located. Default is the `templates` directory [default: templates]
  -r, --registry <REGISTRY>
          Local path or Git URL of the semantic convention registry [default: https://github.com/open-telemetry/semantic-conventions.git]
  -d, --registry-git-sub-dir <REGISTRY_GIT_SUB_DIR>
          Optional path in the Git repository where the semantic convention registry is located [default: model]
```

> Note: The `-d` and `--registry-git-sub-dir` options are only used when the
> registry is a Git URL otherwise these options are ignored.

## registry resolve

```
Resolves a registry

Usage: weaver registry resolve [OPTIONS]

Options:
  -r, --registry <REGISTRY>
          Local path or Git URL of the semantic convention registry

          [default: https://github.com/open-telemetry/semantic-conventions.git]

  -d, --registry-git-sub-dir <REGISTRY_GIT_SUB_DIR>
          Optional path in the Git repository where the semantic convention registry is located

          [default: model]

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
```

> Note: The `-d` and `--registry-git-sub-dir` options are only used when the
> registry is a Git URL otherwise these options are ignored.

## registry update-markdown

```
Update markdown files that contain markers indicating the templates used to update the specified sections

Usage: weaver registry update-markdown [OPTIONS] <MARKDOWN_DIR>

Arguments:
  <MARKDOWN_DIR>  Path to the directory where the markdown files are located

Options:
  -r, --registry <REGISTRY>
          Local path or Git URL of the semantic convention registry to check [default: https://github.com/open-telemetry/semantic-conventions.git]
  -d, --registry-git-sub-dir <REGISTRY_GIT_SUB_DIR>
          Optional path in the Git repository where the semantic convention registry is located [default: model]
      --dry-run
          Whether or not to run updates in dry-run mode
      --attribute-registry-base-url <ATTRIBUTE_REGISTRY_BASE_URL>
          Optional path to the attribute registry. If provided, all attributes will be linked here
  -h, --help
          Print help
```

> Note: The `-d` and `--registry-git-sub-dir` options are only used when the
> registry is a Git URL otherwise these options are ignored.

## registry stats

```
Calculate and display a set of general statistics on a registry (not yet implemented)

Usage: weaver registry stats [OPTIONS]

Options:
  -r, --registry <REGISTRY>
          Local path or Git URL of the semantic convention registry [default: https://github.com/open-telemetry/semantic-conventions.git]
  -d, --registry-git-sub-dir <REGISTRY_GIT_SUB_DIR>
          Optional path in the Git repository where the semantic convention registry is located [default: model]
  -h, --help
          Print help
```

> Note: The `-d` and `--registry-git-sub-dir` options are only used when the
> registry is a Git URL otherwise these options are ignored.
