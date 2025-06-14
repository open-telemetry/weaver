# OpenTelemetry Weaver

<div style="display: flex; align-items: center; margin-bottom: 20px;">
  <img src="docs/images/weaver-logo.svg" alt="OpenTelemetry Weaver" width="220" height="120" style="margin-right: 20px;">
  <div>
    <h3 style="margin: 5px 0;">Observability by Design</h3>
    <p><strong>Treat your telemetry like a public API</strong></p>
  </div>
</div>

[![build](https://github.com/open-telemetry/weaver/actions/workflows/ci.yml/badge.svg)](https://github.com/open-telemetry/weaver/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/open-telemetry/weaver/graph/badge.svg?token=tmWKFoMT2G)](https://codecov.io/gh/open-telemetry/weaver)
[![build](https://github.com/open-telemetry/weaver/actions/workflows/audit.yml/badge.svg)](https://github.com/open-telemetry/weaver/actions/workflows/audit.yml)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Slack](https://img.shields.io/badge/Slack-OpenTelemetry_Weaver-purple)](https://cloud-native.slack.com/archives/C0697EXNTL3)

## What is Observability by Design?

Have you ever experienced:

- Broken alerts after a deployment because metric names changed?
- Complex, hard-to-understand queries due to inconsistent naming?
- Missing critical instrumentation discovered only in production?

**Observability by Design** solves these problems by treating your observability signals (metrics, traces, logs) as a first-class public API that requires the same quality standards as your code.

## Why OpenTelemetry Weaver?

Weaver transforms how you build and maintain observability in your applications by:

- **Preventing Naming Errors**: Generate type-safe client SDKs that eliminate typos and provide IDE auto-completion
- **Ensuring Consistency**: Enforce naming conventions and backward compatibility through policy-based validation
- **Saving Time**: Auto-generate documentation, code, and configuration from a single source of truth
- **Enabling Collaboration**: Create a shared language between developers, SREs, and product managers

## Quick Start

### 1. Install

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

### 2. Define Your Schema

Create `my-app.yaml`:

```yaml
groups:
  - id: my.app.http
    type: metric_group
    brief: HTTP server metrics
    metrics:
      - name: http_requests_total
        metric_name: http.server.requests
        type: counter
        brief: Total HTTP requests
        unit: "1"
        attributes:
          - ref: http.method
          - ref: http.status_code
```

### 3. Generate Type-Safe Code

```bash
# Generate Rust client
weaver registry generate -r ./my-app.yaml -t templates rust

# Generate documentation
weaver registry generate -r ./my-app.yaml -t templates markdown
```

### 4. Use in Your App

```rust
// Generated type-safe client - no more typos!
let counter = metric::Counter::new(
    metrics::HttpServerRequests::NAME,
    metrics::HttpServerRequests::UNIT,
    metrics::HttpServerRequests::DESCRIPTION,
);

// IDE auto-completion for attributes
counter.add(1, &[
    (attributes::HttpMethod::KEY, "GET"),
    (attributes::HttpStatusCode::KEY, 200),
]);
```

## Core Features

### 🔒 **Policy Validation**

Prevent breaking changes and enforce standards:

```bash
weaver registry check --policy policies/
```

### 📊 **Live Validation**

Check running apps against your schema:

```bash
weaver registry live-check --registry ./my-registry
```

### 🔄 **Schema Evolution**

Track changes safely:

```bash
weaver registry diff --baseline-registry v1.0.0
```

### 📝 **Auto Documentation**

Keep docs in sync:

```bash
weaver registry update-markdown docs/
```

## The Observability by Design Workflow

```
Define → Instrument → Validate → Deploy
  ↑_______________________________|
              Iterate
```

1. **Define**: Set clear observability objectives early
2. **Instrument**: Generate type-safe code and docs
3. **Validate**: Catch issues in CI/CD pipeline
4. **Deploy**: Ship with confidence
5. **Iterate**: Refine using production feedback

## OpenTelemetry Semantic Conventions

Weaver leverages the [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/) - 900+ standardized attributes across 74 domains, maintained by expert groups.

Use official conventions, extend them, or create custom registries for your needs.

## Real-World Impact

**Before Weaver:**

```rust
// Developer A:
counter.add(1, [("method", "GET")]);

// Developer B:
counter.add(1, [("http_method", "GET")]);
// Result: Inconsistent data, broken dashboards
```

**With Weaver:**

```rust
// Both developers:
counter.add(1, [(attributes::HttpMethod::KEY, "GET")]);
// Result: Consistent data, reliable observability
```

## What's Next

### Phase 1: Semantic Conventions (Available Now)

- [x] Registry validation and policy enforcement
- [x] Code and documentation generation
- [x] Type-safe client SDKs
- [x] Live instrumentation validation

### Phase 2: Application Telemetry Schema (Coming Soon)

- [ ] Full application observability definitions
- [ ] Automatic schema evolution
- [ ] Native observability tool integration
- [ ] Enhanced dashboard generation

## Getting Help

- **Documentation**: [docs/](docs/)
- **Issues**: [GitHub Issues](https://github.com/open-telemetry/weaver/issues)
- **Discussions**: [OpenTelemetry Slack #weaver](https://cloud-native.slack.com/archives/C0697EXNTL3)

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.

---

_Stop treating observability as an afterthought. Start building it by design._
