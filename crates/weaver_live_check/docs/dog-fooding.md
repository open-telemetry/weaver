# Dog-fooding: Weaver Generates Its Own Code and Documentation

Weaver Live Check uses Weaver's own semantic convention model format and template engine to
define the finding schema, generate Rust types and constants, and generate reference
documentation. This is a dog-fooding exercise that proves Weaver's code generation
capabilities work on real-world models.

## Model

The finding attributes, enumerations, and event are defined as a semantic convention registry
in [`../model/`](../model/):

- **`live_check.yaml`** — Defines the `weaver.finding.*` attributes (including enum types for
  `id`, `level`, `sample_type`, and `signal_type`), template types for `context` and
  `resource_attribute`, and the `weaver.live_check.finding` event.
- **`registry_manifest.yaml`** — Registry manifest declaring the `weaver-live-check` registry
  and its dependency on the upstream OpenTelemetry semantic conventions.

The model uses the version 2 schema format.

## Templates

### Markdown documentation

Weaver Jinja templates at [`../templates/markdown/`](../templates/markdown/)
generate Markdown documentation from the resolved registry:

- **`weaver.yaml`** — Template configuration: single-file output using `filter: .` to pass the
  full resolved registry as context.
- **`live_check_doc.md.j2`** — Main template producing the event overview, attribute summary
  table, and per-attribute detail sections with inline enum value tables and template type info.
- **`macros.j2`** — Reusable macro library for stability badges, attribute tables, enum member
  tables, type display, and example formatting.

### Rust code generation

A single generic Jinja template at [`../templates/rust/`](../templates/rust/) generates all
finding-related Rust types and constants from the model:

- **`weaver.yaml`** — Template configuration: single-file output producing `finding.rs`.
- **`finding.rs.j2`** — Generic template that iterates all `weaver.finding.*` attributes and
  generates:
  - **Attribute name constants** for every attribute (e.g., `WEAVER_FINDING_ID`,
    `WEAVER_FINDING_SAMPLE_TYPE`).
  - **Rust enums** for each attribute with enum members (`FindingId`, `FindingLevel`,
    `SampleType`, `SignalType`). Extensible enums (annotated with `custom_variants: true`
    in the model) get a `Custom(String)` catch-all variant; closed enums derive `Copy`.

The template uses generic heuristics (no hardcoded attribute keys) to derive enum names
from the attribute key structure, and model annotations to control code generation behavior.

## Generating

From the repository root:

### Documentation

```sh
cargo run -- registry generate \
  --registry crates/weaver_live_check/model/ \
  --templates crates/weaver_live_check/templates/ \
  --v2 \
  markdown \
  crates/weaver_live_check/docs/
```

This produces [`finding.md`](finding.md).

### Rust code

```sh
cargo run -- registry generate \
  --registry crates/weaver_live_check/model/ \
  --templates crates/weaver_live_check/templates/ \
  --v2 \
  rust \
  crates/weaver_live_check/src/
```

This produces [`../src/finding.rs`](../src/finding.rs).

## How It Works

1. The `registry generate` command loads the model from `live_check.yaml` and resolves it
   against the registry manifest (including the OTel dependency).
2. The `--v2` flag produces the v2 registry structure where attributes are accessed via
   `ctx.registry.attributes` (with a `key` field) and events via `ctx.registry.events`.
3. The `filter: .` in `weaver.yaml` passes the entire resolved registry as the template context.
4. The Jinja template iterates over all attributes, generating constants for each one and
   enums for those with `type.members`. It derives enum names from the attribute key structure
   and detects extensible enums from model annotations.
5. The generated `finding.rs` module provides `FindingId`, `SampleType`, `SignalType` enums
   and `WEAVER_FINDING_*` constants, replacing hand-written definitions and eliminating
   hardcoded string literals throughout the crate.
