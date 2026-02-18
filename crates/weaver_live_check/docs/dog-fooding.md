# Dog-fooding: Weaver Generates Its Own Documentation

Weaver Live Check uses Weaver's own semantic convention model format and template engine to
define the finding schema and generate reference documentation. This is a dog-fooding exercise
that proves Weaver's code generation capabilities work on real-world models.

## Model

The finding attributes, enumerations, and event are defined as a semantic convention registry
in [`../model/`](../model/):

- **`live_check.yaml`** — Defines the `weaver.finding.*` attributes (including enum types for
  `level`, `sample_type`, and `signal_type`), template types for `context` and
  `resource_attribute`, and the `weaver.live_check.finding` event.
- **`registry_manifest.yaml`** — Registry manifest declaring the `weaver-live-check` registry
  and its dependency on the upstream OpenTelemetry semantic conventions.

The model uses the version 2 schema format.

## Templates

Weaver Jinja templates at [`../templates/markdown/`](../templates/markdown/)
generate Markdown documentation from the resolved registry:

- **`weaver.yaml`** — Template configuration: single-file output using `filter: .` to pass the
  full resolved registry as context.
- **`live_check_doc.md.j2`** — Main template producing the event overview, attribute summary
  table, and per-attribute detail sections with inline enum value tables and template type info.
- **`macros.j2`** — Reusable macro library for stability badges, attribute tables, enum member
  tables, type display, and example formatting.

## Generating the Documentation

From the repository root:

```sh
cargo run -- registry generate \
  --registry crates/weaver_live_check/model/ \
  --templates crates/weaver_live_check/templates/ \
  --v2 \
  markdown \
  crates/weaver_live_check/docs/
```

This produces [`finding.md`](finding.md).

## How It Works

1. The `registry generate` command loads the model from `live_check.yaml` and resolves it
   against the registry manifest (including the OTel dependency).
2. The `--v2` flag produces the v2 registry structure where attributes are accessed via
   `ctx.registry.attributes` (with a `key` field) and events via `ctx.registry.events`.
3. The `filter: .` in `weaver.yaml` passes the entire resolved registry as the template context.
4. The Jinja templates iterate over attributes and events, rendering Markdown with stability
   badges, type information, enum value tables, and formatted examples.
