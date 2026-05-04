# Weaver Macros

Procedural macros for the Weaver CLI.

## `WeaverCommand`

A derive macro for registry command `*Args` structs. It generates the [`CliOverrides`](../weaver_config/src/overrides.rs) trait implementation, which handles the three-layer config merge (defaults → `.weaver.toml` → CLI flags) for each subcommand.
