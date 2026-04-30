# Weaver Config

Project-level configuration for Weaver via `.weaver.toml`.

Discovery walks up from the current working directory to find the first `.weaver.toml` file (like `.rustfmt.toml`). The `--config` CLI option on `live-check` overrides discovery.

Currently scoped to the `live-check` command:

- All `live-check` CLI flags (`input_source`, `input_format`, `format`, `templates`, `no_stream`, `no_stats`, `output`, `advice_policies`, `advice_preprocessor`) plus the `[live_check.otlp]` and `[live_check.emit]` sub-tables.
- `[[live_check.finding_filters]]` for dropping findings by ID, minimum level, sample name, and signal type.

CLI flags always take precedence over config values; config values take precedence over hardcoded defaults.

See the [Finding Filters](../weaver_live_check/README.md#finding-filters) section in the live-check README for usage details, and `schemas/weaver-config.json` for the full JSON schema.
