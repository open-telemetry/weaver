# OpenTelemetry Weaver (status: Prototype)

## Overview

> At this stage, the project is being used as a **Proof of Concept** to explore and
> refine the 'Application Telemetry Schema: Vision and Roadmap' [OTEP](https://github.com/open-telemetry/oteps/blob/main/text/0243-app-telemetry-schema-vision-roadmap.md),
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
  -h, --help      Print help
  -V, --version   Print version
```

### Command `registry`

This command provides subcommands to manage semantic convention registries.

```
Manage Semantic Convention Registry

Usage: weaver registry <COMMAND>

Commands:
  check     Validates a registry (i.e., parsing, resolution of references, extends clauses, and constraints)
  generate  Generates documentation or code for a registry (not yet implemented)
  resolve   Resolves a registry (not yet implemented)
  search    Searches a registry (not yet implemented)
  stats     Calculate and display a set of general statistics on a registry (not yet implemented)
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### Command `search` (Experimental)

This command provides an interactive terminal UI, allowing users to search for
attributes and metrics specified within a given semantic convention registry or
a telemetry schema (including dependencies).

To search into the OpenTelemetry Semantic Convention Registry, run the following
command:

```bash
weaver search registry https://github.com/open-telemetry/semantic-conventions.git model 
```

To search into a telemetry schema, run the following command:

```bash
weaver search schema demo/app-telemetry-schema.yaml
```

This search engine leverages [Tantivy](https://github.com/quickwit-oss/tantivy)
and supports a simple [search syntax](https://docs.rs/tantivy/latest/tantivy/query/struct.QueryParser.html)
in the search bar.

### Command `resolve` (Experimental)

This command resolves a schema or a semantic convention registry (not yet
implemented) and displays the result on the standard output.
Alternatively, the result can be written to a file if specified using the
`--output` option. This command is primarily used for validating and debugging
telemetry schemas and semantic convention registries.

```bash
weaver resolve schema telemetry-schema.yaml --output telemetry-schema-resolved.yaml
```

A "resolved schema" is one where:
- All references have been resolved and expanded.
- All overrides have been applied.
- This resolved schema is what the code generator and upcoming plugins utilize.

### Command `gen-client` (Experimental)

This command generates a client SDK from a telemetry schema for a given language
specified with the `--language` option.

```bash
weaver gen-client --schema telemetry-schema.yaml --language go
```

In the future, users will be able to specify the protocol to use for the generated
client SDK (i.e. OTLP or OTel Arrow Protocol) and few others options.

### Command `languages` (Experimental)

This command displays all the languages for which a client SDK/API can
be generated.

```bash
weaver languages
```

### Crates Layout

This project utilizes the concept of a cargo workspace to organize the
libraries developed for the OTel Weaver project. The set of crates in the
workspace is grouped under the directory `crates/*`. Adding a crate under this
directory will automatically include it in the OTel Weaver project due to the
directive `members = [ "crates/*" ]` included in the main `Cargo.toml` under the
`[workspace]` section.

This project has not been published to crates.io and will not be until it is
ready for production use.

Every crate in the workspace must have a `README.md` file that describes the
purpose of the crate and how to use it. Furthermore, the name of each crate
must be prefixed with `weaver_` to avoid any conflicts with existing crates on
crates.io.

The following is a list of crates in the workspace, along with a brief
description and the current status of each crate:

| Crate                                                             | Description                                             | Status                 |
|-------------------------------------------------------------------|---------------------------------------------------------|------------------------|
| [weaver_semconv](crates/weaver_semconv/README.md)                 | Semantic Convention Registry Data Model                 | Alpha; Need more tests |
| [weaver_version](crates/weaver_version/README.md)                 | OpenTelemetry Schema Versioning Data Model              | Alpha; Need more tests |
| [weaver_resolved_schema](crates/weaver_resolved_schema/README.md) | Resolved Schema Data Model                              | Work-In-Progress       |
| [weaver_schema](crates/weaver_schema/README.md)                   | Telemetry Schema Data Model                             | Work-In-Progress       |
| [weaver_resolver](crates/weaver_resolver/README.md)               | Telemetry Schema Resolution Process                     | Work-In-Progress       |
| [weaver_cache](crates/weaver_cache/README.md)                     | Telemetry Schema and Semantic Convention Registry Cache | Work-In-Progress       |
| [weaver_logger](crates/weaver_logger/README.md)                   | Generic logger supported colorized output               | Alpha                  |
| [weaver_template](crates/weaver_template/README.md)               | Functions and Filters used in the template engine       | Work-In-Progress       |

Note 1: Alpha status means that the crate is in a usable state but may have
limited functionality and/or may not be fully tested. 

Note 2: Work-In-Progress status means that the crate is still under active
development.

### Architecture

The OpenTelemetry Weaver tool is architecturally designed as a platform. By default, this
tool incorporates a template engine that facilitates Client SDK/API generation
across various programming languages. In the future, we plan to integrate a
WASM plugin system, allowing the community to enhance the platform. This would
pave the way for features like enterprise data catalog integration, privacy policy enforcement,
documentation generation, dashboard creation, and more.

Below is a diagram detailing the primary components of the OpenTelemetry Weaver tool.

![OpenTelemetry Weaver Platform](docs/images/otel-weaver-platform.png)

## Links

Internal links:
- [Component Telemetry Schema](docs/component-telemetry-schema.md) (proposal)
- [Resolved Telemetry Schema](docs/resolved-telemetry-schema.md) (proposal)
- [Internal crates interdependencies](docs/dependencies.md)
- [Change log](CHANGELOG.md)

External links:
- Application Telemetry Schema: Vision and Roadmap - [PR](https://github.com/open-telemetry/oteps/pull/243)
- OpenTelemetry Telemetry Schema v1.2.0 [Draft](https://github.com/lquerel/oteps/blob/app-telemetry-schema-format/text/0241-telemetry-schema-ext.md) (not yet ready).
- [OpenTelemetry Semantic Convention File Format](https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/syntax.md)
- [OpenTelemetry Schema File Format v1.1.0](https://opentelemetry.io/docs/specs/otel/schemas/file_format_v1.1.0/)
- Presentation slides from the Semantic Convention SIG meeting on October 23, 2023 [here](https://docs.google.com/presentation/d/1nxt5VFlC1mUjZ8eecUYK4e4SxThpIVj1IRnIcodMsNI/edit?usp=sharing).
- Meta/Facebook's [positional paper](https://research.facebook.com/publications/positional-paper-schema-first-application-telemetry/) 
  presenting a similar approach but based on Thrift+Annotations+Automations.

## Contributing

Pull requests are welcome. For major changes, please open an issue
first to discuss what you would like to change. For more information, please
read [CONTRIBUTING](CONTRIBUTING.md).


## License

OpenTelemetry Weaver is licensed under Apache License Version 2.0.
