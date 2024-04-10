# OpenTelemetry Weaver (status: Prototype)

[![build](https://github.com/open-telemetry/weaver/actions/workflows/ci.yml/badge.svg)](https://github.com/open-telemetry/weaver/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/open-telemetry/weaver/graph/badge.svg?token=tmWKFoMT2G)](https://codecov.io/gh/open-telemetry/weaver)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
----
[Install](#install) | [Usage](#usage) | [Architecture](docs/architecture.md) | [Change log](CHANGELOG.md) | [Contributing](CONTRIBUTING.md) | [Links](#links) | 


## Overview

> At this stage, the project is being used as a **Proof of Concept** to explore and
> refine the 'Application Telemetry Schema: Vision and
> Roadmap' [OTEP](https://github.com/open-telemetry/oteps/blob/main/text/0243-app-telemetry-schema-vision-roadmap.md),
> which has been approved and merged.
>
> This project is a **work in progress and is not ready for production use**.

OpenTelemetry Weaver is a CLI tool that enables users to:

- Manage Semantic Convention Registries: check, generate, resolve, search, stats commands.
- Manage Telemetry Schemas: check, generate, resolve, search, stats commands.

Note: Telemetry Schema commands are only available with the --features experimental flag.

## Install

Currently, there is no binary distribution available. To install the tool, you
must build it from source. To do so, you need to have Rust installed on your
system (see [Install Rust](https://www.rust-lang.org/tools/install)).

To build the tool:

- In debug mode, run the following command:
  ```
  cargo build
  ```
- In release mode, run the following command:
  ```
  cargo build --release
  ```

The generated `weaver` binary will be located in the `target/debug` directory
for debug mode or the `target/release` directory for release mode.

## Usage

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

### Command `registry`

This command provides subcommands to manage semantic convention registries.

```
Manage Semantic Convention Registry

Usage: weaver registry <COMMAND>

Commands:
  check            Validates a registry (i.e., parsing, resolution of references, extends clauses, and constraints)
  generate         Generates artifacts from a registry
  resolve          Resolves a registry
  search           Searches a registry (not yet implemented)
  stats            Calculate and display a set of general statistics on a registry (not yet implemented)
  update-markdown  Update markdown files that contain markers indicating the templates used to update the specified sections

Options:
  -h, --help  Print help
```

### Sub-Command `registry check`

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

### Sub-Command `registry generate`

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

### Sub-Command `registry resolve`

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

## Documentation

- [Architecture](docs/architecture.md) of the weaver project.
- [Weaver Force](docs/template-engine.md): an integrated template engine to generate
documentation and code from semantic conventions and application telemetry schemas.
- [Weaver Checker](crates/weaver_policy_engine/README.md): an integrated policy
engine to enforce policies on telemetry data.
- Application Telemetry Schema: [Vision and Roadmap](https://github.com/open-telemetry/oteps/blob/main/text/0243-app-telemetry-schema-vision-roadmap.md)
- Presentation slides from the Semantic Convention SIG meeting on October 23,
  2023 [here](https://docs.google.com/presentation/d/1nxt5VFlC1mUjZ8eecUYK4e4SxThpIVj1IRnIcodMsNI/edit?usp=sharing).

## Experimental
- [Component Telemetry Schema](docs/component-telemetry-schema.md) (proposal)
- [Resolved Telemetry Schema](docs/resolved-telemetry-schema.md) (proposal)
- OpenTelemetry Telemetry Schema
  v1.2.0 [Draft](https://github.com/lquerel/oteps/blob/app-telemetry-schema-format/text/0241-telemetry-schema-ext.md) (
  not yet ready).

## Links
- [OpenTelemetry Semantic Convention File Format](https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/syntax.md)
- [OpenTelemetry Schema File Format v1.1.0](https://opentelemetry.io/docs/specs/otel/schemas/file_format_v1.1.0/)
- Meta/Facebook's [positional paper](https://research.facebook.com/publications/positional-paper-schema-first-application-telemetry/)
presenting a similar approach but based on Thrift+Annotations+Automations.

## Contributing

Pull requests are welcome. For major changes, please open an issue
first to discuss what you would like to change. For more information, please
read [CONTRIBUTING](CONTRIBUTING.md).

## License

OpenTelemetry Weaver is licensed under Apache License Version 2.0.
