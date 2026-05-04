# Weaver Macros

Procedural macros for the Weaver CLI.

## `WeaverCommand`

A derive macro for registry command `*Args` structs. It generates the [`CliOverrides`](../weaver_config/src/overrides.rs) trait implementation, which handles the three-layer config merge (defaults → `.weaver.toml` → CLI flags) for each subcommand.

## Inspecting macro output

Use [`cargo-expand`](https://github.com/dtolnay/cargo-expand) to see the code the macro generates:

```bash
cargo install cargo-expand  # one-time setup; also requires `rustup toolchain install nightly`

cargo expand registry::emit                    # expand a whole module
cargo expand registry::emit::RegistryEmitArgs  # expand a specific item
```

The output is formatted Rust showing the exact `impl CliOverrides`, generated config struct, and `apply_overrides` fn.
