# OpenTelemetry Weaver

[![build](https://github.com/open-telemetry/weaver/actions/workflows/ci.yml/badge.svg)](https://github.com/open-telemetry/weaver/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/open-telemetry/weaver/graph/badge.svg?token=tmWKFoMT2G)](https://codecov.io/gh/open-telemetry/weaver)
[![build](https://github.com/open-telemetry/weaver/actions/workflows/audit.yml/badge.svg)](https://github.com/open-telemetry/weaver/actions/workflows/audit.yml)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Slack](https://img.shields.io/badge/Slack-OpenTelemetry_Weaver-purple)](https://cloud-native.slack.com/archives/C0697EXNTL3)
----

[Getting started](#getting-started) | [Main commands](#main-commands) | [Generate Doc & Code](crates/weaver_forge/README.md) | [Architecture](docs/architecture.md) | [Change log](CHANGELOG.md) | [Contributing](CONTRIBUTING.md) | [Links](#links) |

> [!NOTE]
> Codegen authors, please refer to the following documentation to learn how to
> use Weaver for generating code from semantic conventions:
> - [Templates organization, integrated Jinja2 engine, list of supported filters/functions/tests](crates/weaver_forge/README.md)
> - [Weaver Configuration](docs/weaver-config.md)

## What is OpenTelemetry Weaver?

**OTel Weaver** is a comprehensive tool designed to enable developers to
easily develop, validate, document, and deploy semantic conventions (phase 1)
and application telemetry schemas (phase 2). As an **open, customizable, and
extensible platform**, it aims to serve both as a standalone developer tool
and as an integral component within CI/CD pipelines—whether for the
OpenTelemetry project itself, other open-source projects, vendor solutions,
or even large-scale enterprise deployments leveraging OpenTelemetry.

## Semantic Conventions and Application Telemetry Schema

- **Semantic conventions** enable SMEs to define a catalog of well-defined and reusable
  attributes and signals. OpenTelemetry maintains an official Semantic Convention
  Registry that any project can leverage for consistent instrumentation.
  Open-source projects, vendors, and enterprises can also implement their own
  registries for specific needs, which Weaver can import and resolve to cover all
  instrumented components of complex systems.
- **Application Telemetry Schema** allows developers to specify the semantic
  convention registries and custom attributes and signals supported by their
  applications. The vision behind this concept is detailed in this [document](https://github.com/open-telemetry/oteps/blob/main/text/0243-app-telemetry-schema-vision-roadmap.md),
  with implementation planned for Weaver's phase 2.

## Design Principles

Weaver is built upon principles of extensibility, customizability, scalability,
robustness, reliability, and cross-platform compatibility.

## Key Features

- **Schema Resolution**: The Weaver Resolver sub-project resolves references,
  extends clauses, and overrides in semantic convention registries and application
  telemetry schemas, producing self-contained, easy-to-process, and shareable
  resolved schemas.
- **Policy Enforcement**: The Weaver Checker sub-project ensures the quality,
  maintainability, and extensibility of registries and schemas by checking them
  against a set of declarative policies using the popular rego policy language.
- **Documentation and Code Generation**: The Weaver Forge sub-project generates
  documentation and code from registries and schemas. It leverages a `jq-compatible`
  expression language for data transformation and a `jinja2-compatible` template
  engine for output generation.
- **WASM-based Plugin System (future plan)**: A plugin system based on WASM will
  be implemented to extend the Weaver platform. Plugins could be designed to
  download registries and schemas from custom systems, feed data catalog solutions,
  or configure dashboard systems, among other functionalities.

## Scalability

- Built with Rust, Weaver offers performance comparable to C or C++ implementation.
  The entire OpenTelemetry semantic convention registry can be downloaded, resolved,
  and documented in under 2 seconds.
- Semantic Convention Registry Git repositories can be efficiently cached locally.
- Registry and schema validation, as well as documentation and code generation,
  are parallelized for optimal performance.

## Robustness and Reliability

- **Memory Safety**: Rust ensures memory safety, preventing common vulnerabilities.
- **Comprehensive Error Reporting**: Weaver reports as many errors as possible in
  a single pass, providing developers with comprehensive feedback.
- **Quality Assurance**: Code coverage, Cargo deny, Dependabot, and automated security
  audits.

## Cross-Platform Compatibility

- Tested Platforms: Weaver is manually tested on Linux, macOS, and Windows.
- Future Plans: Automated tests will be implemented for broader platform coverage.

## Getting started

Currently, weaver can be consumed in one of three ways:

- [Pre-built Binaries](#pre-built-binaries)
- [Docker Image](#docker-image)
- [Building from source](#building-from-source)

### Pre-built Binaries

Weaver release attach pre-built binaries for supported platforms with every release.
Instructions for installing are included in these release notes.

See: [Weaver Releases](https://github.com/open-telemetry/weaver/releases)

### Docker Image

Weaver deploys a docker image for development to [docker hub](https://hub.docker.com/r/otel/weaver).

Instructions for using the docker image can be found [here](docs/docker-guide.md).

### Building from source

To install the tool from source. you need to have Rust installed on your
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

To run a registry check, use the following command:
```
cargo run -- registry check
```

This command will check the OpenTelemetry Semantic Convention Registry by
default.

To check a set of policies against the registry, use the following command:
```
cargo run -- registry check -b path/to/policies
```

An example of a policy file can be found here [schemas/otel_policies.rego](schemas/otel_policies.rego).

## Main commands

In phase 1, the only supported commands are related to the management of
Semantic Convention Registries. The following commands are available:

| Command                                                                   | Description                                 |
|---------------------------------------------------------------------------|---------------------------------------------|
| [weaver registry check](docs/usage.md#registry-check)                     | Check the validity of a semconv registry    |
| [weaver registry resolve](docs/usage.md#registry-resolve)                 | Resolve a semconv registry                  |
| [weaver registry diff](docs/usage.md#registry-diff)                       | Generate a diff report between two versions |
| [weaver registry generate](docs/usage.md#registry-generate)               | Generate artifacts from a semconv registry  |
| [weaver registry update-markdown](docs/usage.md#registry-update-markdown) | Update semconv snippet-based markdown files |
| [weaver registry stats](docs/usage.md#registry-stats)                     | Generate statistics on a semconv registry   |

Phase 2 will introduce commands related to the management of Application
Telemetry Schemas.

## Documentation

- [Weaver Architecture](docs/architecture.md): A document detailing the architecture of the project.
- [Weaver Configuration](docs/weaver-config.md): A document detailing the configuration options available.
- [Weaver Forge](crates/weaver_forge/README.md): An integrated template engine designed to generate
  documentation and code based on semantic conventions.
- [Weaver Checker](crates/weaver_checker/README.md): An integrated policy
  engine for enforcing policies on semantic conventions.
- [Schema Changes](docs/schema-changes.md): A document describing the data model
  used to represent the differences between two versions of a semantic convention registry.
- [Application Telemetry Schema OTEP](https://github.com/open-telemetry/oteps/blob/main/text/0243-app-telemetry-schema-vision-roadmap.md):
  A vision and roadmap for the concept of Application Telemetry Schema.
- Presentation slides from the Semantic Convention SIG meeting on October 23,
  2023 [here](https://docs.google.com/presentation/d/1nxt5VFlC1mUjZ8eecUYK4e4SxThpIVj1IRnIcodMsNI/edit?usp=sharing).

## Examples of Code and Documentation Generation

The following examples show how to generate code and documentation for various languages using Weaver.

> Note: A more out of the box experience will be available in the future.

**Java**
From GitHub repo main branch.

```bash
weaver registry generate \
--templates https://github.com/open-telemetry/semantic-conventions-java.git[buildscripts/templates] \
java
```

**C++**
From a specific release using the corresponding archive.

```bash
weaver registry generate \
--templates https://github.com/open-telemetry/opentelemetry-cpp/archive/refs/tags/v1.20.0.zip[buildscripts/semantic-convention/templates] \
./ \
--param filter=any \
--param output=output/ \
--param schema_url=https://opentelemetry.io/schemas/v1.32.0
```

**Python**
From GitHub repo main branch.

```bash
weaver registry generate \
--templates https://github.com/open-telemetry/opentelemetry-python.git[scripts/semconv/templates] \
--param output=./ \
--param filter=any \
./
```

**Markdown (attribute registry)**
From a specific release using a GitHub archive release.

```bash
weaver registry generate \
--templates https://github.com/open-telemetry/semantic-conventions/archive/refs/tags/v1.32.0.zip[templates] \
markdown
```

**Templates in local path**

```bash
weaver registry generate \
--templates custom-templates/ \
markdown
```

## Experimental

- [Component Telemetry Schema](docs/component-telemetry-schema.md) (proposal)
- [Resolved Telemetry Schema](docs/resolved-telemetry-schema.md) (proposal)
- OpenTelemetry Telemetry Schema
  v1.2.0 [Draft](https://github.com/lquerel/oteps/blob/app-telemetry-schema-format/text/0241-telemetry-schema-ext.md) (
  not yet ready).

## Shell Completions

Weaver supports shell completions via
[clap_complete](https://docs.rs/clap_complete/latest/clap_complete/) for the
following shells: bash, elvish, fish, powershell, and zsh. Installing and using
the completion depends on your shell, and your setup.

For example, to install the completion for bash, you can run the following
command:

```bash
weaver completion bash > $BASH_COMPLETION_USER_DIR/weaver
```

To enable the completion for zsh, you can run the following command:

```bash
weaver completion zsh > ${fpath[1]}/_weaver
```

That will install the completion in the first directory in your `fpath` array.

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
