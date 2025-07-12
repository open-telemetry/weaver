# OpenTelemetry Weaver

<p align="left">
  <img src="docs/images/weaver-logo.svg" alt="OpenTelemetry Weaver" width="200" height="100" align="left" style="margin-right: 20px;">
</p>

### Observability by Design

_Treat your telemetry like a public API_

&nbsp;

[![build](https://github.com/open-telemetry/weaver/actions/workflows/ci.yml/badge.svg)](https://github.com/open-telemetry/weaver/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/open-telemetry/weaver/graph/badge.svg?token=tmWKFoMT2G)](https://codecov.io/gh/open-telemetry/weaver)
[![build](https://github.com/open-telemetry/weaver/actions/workflows/audit.yml/badge.svg)](https://github.com/open-telemetry/weaver/actions/workflows/audit.yml)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Slack](https://img.shields.io/badge/Slack-OpenTelemetry_Weaver-purple)](https://cloud-native.slack.com/archives/C0697EXNTL3)

OpenTelemetry Weaver helps teams build observability by design, enabling consistent, type-safe, and automated telemetry through semantic conventions. With Weaver, you can define, validate, and evolve your telemetry schemas, ensuring reliability and clarity across your systems.

## What is Observability by Design?

Have you ever experienced:

- Broken alerts after a deployment because metric names changed?
- Complex, hard-to-understand queries due to inconsistent naming?
- Teams struggling to interpret unclear or undocumented signals?
- Missing critical instrumentation discovered only in production?

**Observability by Design** solves these problems by treating your observability signals (metrics, traces, logs) as a first-class public API that requires the same quality standards as your code.

An introduction to Weaver and Observability by Design is presented in the official blog post: [Observability by Design: Unlocking Consistency with OpenTelemetry Weaver](https://opentelemetry.io/blog/2025/otel-weaver/)

## Install

**Pre-built binaries:**

Linux, Windows and Mac installers on the [releases](https://github.com/open-telemetry/weaver/releases) page.

**Docker:**

```bash
docker pull otel/weaver
```

**From source:**

```bash
git clone https://github.com/open-telemetry/weaver.git
cd weaver
cargo build --release
```

## Examples and How-Tos

- [O11y by design](https://github.com/jsuereth/o11y-by-design/) - from the CNCF 2025 presentation
- [Weaver Example](https://github.com/jerbly/weaver-example) - doc-gen, code-gen, emit, live-check in CI/CD
- [Define your own telemetry schema](docs/define-your-own-telemetry-schema.md) - A guide on how to define your own
  telemetry schema using semantic conventions.


## Media

- [OpenTelemetry Weaver - Observability by Design](https://www.youtube.com/watch?v=BJt6LyJEYD0) - CNCF presentation introducing Weaver's core concepts
- [OpenTelemetry Semantic Conventions and How to Avoid Broken Observability](https://www.youtube.com/watch?v=Vd6MheRkHss) - SRECON Americas 2025 presentation
- [Observability by Design: Unlocking Consistency with OpenTelemetry Weaver](https://opentelemetry.io/blog/2025/otel-weaver/) - official blog post on opentelemetry.io
- [Presentation slides from the Semantic Convention SIG meeting on October 23, 2023](https://docs.google.com/presentation/d/1nxt5VFlC1mUjZ8eecUYK4e4SxThpIVj1IRnIcodMsNI/edit?usp=sharing).

## Main Commands

| Command                                                                   | Description                                 |
|---------------------------------------------------------------------------|---------------------------------------------|
| [weaver registry check](docs/usage.md#registry-check)                     | Validates a semantic convention registry    |
| [weaver registry resolve](docs/usage.md#registry-resolve)                 | Resolves a semantic convention registry     |
| [weaver registry diff](docs/usage.md#registry-diff)                       | Generate a diff between two versions of a semantic convention registry |
| [weaver registry generate](docs/usage.md#registry-generate)               | Generates artifacts from a semantic convention registry  |
| [weaver registry update-markdown](docs/usage.md#registry-update-markdown) | Update markdown files that contain markers indicating the templates used to update the specified sections |
| [weaver registry live-check](docs/usage.md#registry-live-check)           | Check the conformance level of an OTLP stream against a semantic convention registry |
| [weaver registry emit](docs/usage.md#registry-emit)                       | Emits a semantic convention registry as example signals to your OTLP receiver |
| [weaver completion](docs/usage.md#completion)                             | Generate shell completions |

## Documentation

- [Weaver Architecture](docs/architecture.md): A document detailing the architecture of the project.
- [Weaver Configuration](docs/weaver-config.md): A document detailing the configuration options available.
- [Weaver Forge](crates/weaver_forge/README.md): An integrated template engine designed to generate documentation and code based on semantic conventions.
- [Weaver Checker](crates/weaver_checker/README.md): An integrated policy engine for enforcing policies on semantic conventions.
- [Weaver Live-check](crates/weaver_live_check/README.md): Live check is a developer tool for assessing sample telemetry and providing advice for improvement.
- [Schema Changes](docs/schema-changes.md): A document describing the data model used to represent the differences between two versions of a semantic convention registry.
- [Application Telemetry Schema OTEP](https://github.com/open-telemetry/oteps/blob/main/text/0243-app-telemetry-schema-vision-roadmap.md): A vision and roadmap for the concept of Application Telemetry Schema.

## Getting Help

- **Documentation**: [docs/](docs/)
- **Issues**: [GitHub Issues](https://github.com/open-telemetry/weaver/issues)
- **Roadmap**: [Github Project](https://github.com/orgs/open-telemetry/projects/74)
- **Discussions**: [OpenTelemetry Slack #weaver](https://cloud-native.slack.com/archives/C0697EXNTL3)

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.

---

_Stop treating observability as an afterthought. Start building it by design._
