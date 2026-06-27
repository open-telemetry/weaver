# Weaver Config

Project-level configuration for Weaver via `.weaver.toml`.

## Discovery and loading

Weaver walks up from the current working directory to find the first `.weaver.toml` file (like `.rustfmt.toml`). The `--config` CLI option on `live-check` can override discovery.

Configuration is applied in three layers, each overriding the previous:

1. **Hardcoded defaults** — defined in each command's generated `XxxConfig` struct
2. **`.weaver.toml`** — command-specific sections (e.g. `[emit]`, `[generate]`)
3. **CLI flags** — always win when provided

## `.weaver.toml` structure

```toml
#:schema schemas/weaver-config.json

# Shared registry settings — applied to all subcommands that accept a registry.
[registry]
path = "path/to/registry"
v2 = true

# Shared policy settings.
[policy]
skip = true

# Shared diagnostic output settings.
[diagnostics]
format = "ansi"   # ansi | json | gh_workflow_command

# Per-command sections — each matches the CLI subcommand name.
[emit]
stdout = true
endpoint = "http://localhost:4317"

[generate]
templates = "path/to/templates"
target = "rust"
output = "output"

[stats]
format = "text"

[diff]
format = "ansi"

[check]
# (no fields — check has no command-specific config)

[mcp]
namespace_separator = "."

[infer]
output = "./inferred-registry/"
grpc_port = 4317

[package]
output = "output"

[update-markdown]
markdown_dir = "docs"
target = "markdown"
dry_run = false

[serve]
bind = "127.0.0.1:8080"

# Live-check has a richer nested config.
[live_check]
format = "json"
input_source = "stdin"

[[live_check.finding_filters]]
exclude = ["missing_namespace"]
```

See `schemas/weaver-config.json` for the full JSON schema (with VS Code / taplo completion support via the `#:schema` annotation above).

## Architecture

### `WeaverConfig`

The top-level config type. Typed fields cover the cross-cutting sections (`registry`, `policy`, `diagnostics`, `live_check`, `auth`). Per-command sections are stored as a raw `toml::Table` via `#[serde(flatten)]` and deserialized on demand by `command_config<C>(section)`.

### `CliOverrides` trait

Each command's `*Args` struct implements `CliOverrides`, which declares:

- `type Config` — the command-specific config struct (e.g. `EmitConfig`)
- `SUBCOMMAND` — the CLI subcommand name (e.g. `"emit"`), used for test introspection
- `extract_config` — deserializes the section from `WeaverConfig`
- `apply_overrides` — writes `Some` CLI values onto the config
- `excluded_args` / `config_only_fields` — drive the consistency test
- `apply_registry_overrides` / `apply_policy_overrides` / `apply_diagnostic_overrides` — handle shared args

`load_config(args, cfg)` executes the three-layer merge and returns `CommandConfig<C>` containing the merged command config plus effective registry, policy, and diagnostic configs.

### `#[derive(WeaverCommand)]` macro

The `weaver_macros` proc-macro generates the `XxxConfig` struct and the full `CliOverrides` impl from field annotations on the `*Args` struct. This eliminates boilerplate and keeps the config definition co-located with the CLI definition.

## Adding a new command

### 1. Create `src/registry/{name}.rs`

Annotate the Args struct with `#[derive(WeaverCommand)]`:

```rust
use weaver_config::{WeaverCommand, WeaverConfig};
use weaver_macros::weaver_command;  // proc_macro_attribute — must import directly, cannot re-export

// IMPORTANT: #[weaver_command] must appear BEFORE #[derive(...)].
// It is a proc-macro attribute that injects `[default: val]` doc comments so
// clap shows them in --help. Because attributes run in source order, placing
// #[derive] first means Args sees the original struct before injection happens.
#[weaver_command(section = "my-command")]  // must match the CLI subcommand name (kebab-case)
#[derive(Debug, Args, WeaverCommand)]
pub struct RegistryMyCommandArgs {
    /// Parameters to specify the semantic convention registry.
    #[command(flatten)]
    #[shared(registry)]          // generates apply_registry_overrides
    registry: RegistryArgs,

    /// Policy parameters.
    #[command(flatten)]
    #[shared(policy)]            // omit and add `no_policy` to #[weaver_command] if unused
    policy: PolicyArgs,

    /// Diagnostic output parameters.
    #[command(flatten)]
    #[shared(diagnostic)]
    pub diagnostic: DiagnosticArgs,

    /// A string option with a default. Doc comment becomes the JSON Schema description.
    #[arg(long)]
    #[config(default = "text")]
    pub format: Option<String>,

    /// An optional path with no default.
    #[arg(short, long)]
    #[config]
    pub output: Option<PathBuf>,

    /// CLI-only flag — not persisted to config.
    #[arg(long)]
    pub dry_run: Option<bool>,

    /// Config-only field (no CLI flag) — useful for values only meaningful in .weaver.toml.
    #[config_only(default = "output")]
    pub output_dir: Option<String>,
}
```

The macro generates:

```rust
// XxxConfig struct (name derived from section: "my-command" → MyCommandConfig)
#[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq)]
#[serde(default)]
pub struct MyCommandConfig {
    pub format: String,           // unwrapped from Option because default is set
    pub output: Option<PathBuf>,  // kept as Option because no default
}

impl Default for MyCommandConfig { ... }
impl CliOverrides for RegistryMyCommandArgs { ... }
```

### Field annotations

| Annotation | Effect on generated `XxxConfig` | Notes |
|---|---|---|
| `#[config(default = "value")]` | `field: T` with the given default | Unwraps `Option<T>` to `T` |
| `#[config]` | `field: Option<T>`, default `None` | Keeps `Option<T>` |
| `#[config_only(default = "value")]` | Same as `#[config(default)]` | Field absent from CLI; used for positional/config-only values |
| `#[config_only]` | `field: Option<T>`, default `None` | Config-only, no CLI flag |
| *(none)* | Not in config | CLI-only; auto-added to `excluded_args()` |
| `#[shared(registry)]` | Generates `apply_registry_overrides` | Used on flattened `RegistryArgs` |
| `#[shared(policy)]` | Generates `apply_policy_overrides` | Used on flattened `PolicyArgs` |
| `#[shared(diagnostic)]` | Generates `apply_diagnostic_overrides` | Used on flattened `DiagnosticArgs` |

### Defining defaults: `#[config(default = ...)]` not `#[arg(default_value = ...)]`

**Never use clap's `#[arg(default_value = "...")]` on a field that is also annotated with `#[config(...)]`.**

The three-layer merge works because `apply_overrides` uses `override_if_set!`, which only overwrites the config value when the CLI field is `Some`. If you attach a clap default, clap populates the field unconditionally, making it always `Some` — so the CLI layer wins over `.weaver.toml` every time, silently ignoring the config file.

```rust
// CORRECT: default lives in the config layer
#[arg(long)]
#[config(default = "text")]
pub format: Option<String>,

// WRONG: clap default means CLI always wins, .weaver.toml is ignored
#[arg(long, default_value = "text")]   // ← do not do this
#[config(default = "text")]
pub format: Option<String>,
```

The `#[config(default = "value")]` annotation generates the `Default` impl for `XxxConfig`, placing the default in layer 1 (hardcoded defaults) where it belongs. The CLI arg stays `Option<T>` with no clap default so it only overrides when the user explicitly passes the flag.

Add `no_policy` to `#[weaver_command(...)]` for commands that don't use the policy engine (e.g. `#[weaver_command(section = "stats", no_policy)]`).

### 2. Write the command function

```rust
pub(crate) fn command(
    args: &RegistryMyCommandArgs,
    cfg: Option<&WeaverConfig>,
    auth: &HttpAuthResolver,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let cmd_config = load_config(args, cfg);
    // cmd_config.config   → MyCommandConfig (merged defaults + toml + CLI)
    // cmd_config.registry → EffectiveRegistryConfig
    // cmd_config.policy   → EffectivePolicyConfig
    ...
}
```

### 3. Register in `src/registry/mod.rs`

```rust
mod my_command;
// ...
enum RegistrySubCommand {
    // ...
    MyCommand(my_command::RegistryMyCommandArgs),
}
// ...
RegistrySubCommand::MyCommand(args) => CmdResult::new(
    my_command::command(args, cfg, auth),
    args.diagnostic.to_effective(cfg),
),
```

### 4. Update the JSON schema

Add the new Config type to `WeaverConfigSchema` in `src/registry/json_schema.rs`:

```rust
struct WeaverConfigSchema {
    // ... existing fields ...
    pub my_command: super::my_command::MyCommandConfig,
}
```

Then regenerate `schemas/weaver-config.json`:

```sh
cargo run -- registry json-schema --json-schema weaver-config -o schemas/weaver-config.json
```

### 5. Add the consistency test

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_config_cli_consistency() {
        use crate::registry::tests::assert_config_cli_consistency;
        assert_config_cli_consistency::<RegistryMyCommandArgs>();
    }
}
```

This test verifies that every field in `MyCommandConfig` has a corresponding CLI arg (or is listed in `config_only_fields()`), and every CLI arg is either covered by the config or listed in `excluded_args()`. It runs automatically as part of `cargo nextest run`.

### Special cases

- **`SocketAddr` fields**: the macro automatically adds `#[schemars(with = "String")]` so the JSON schema renders it as a string.
- **Template paths** (`Option<String>` pointing to a directory or archive): parse via `VirtualDirectoryPath::from_str` in the command body — a `.zip` suffix is treated as a local archive, local paths become `LocalFolder`, and HTTP(S) URLs become remote sources.
- **Custom template context**: if your command generates a custom context struct (rather than the standard registry template schema), build it explicitly in your `match resolved { ... }` arms instead of calling `v.template_schema()`. See `src/registry/stats.rs` for an example.
