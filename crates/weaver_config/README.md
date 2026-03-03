# Weaver Config

Project-level configuration for Weaver via `.weaver.toml`.

Discovery walks up from the current working directory to find the first `.weaver.toml` file (like `.rustfmt.toml`). The `--config` CLI option overrides discovery.

Currently supports live-check finding overrides and filters. Intended to be extended to cover all Weaver CLI configuration in a future release.

See the [Finding Modification](../weaver_live_check/README.md#finding-modification) section in the live-check README for usage details.
