// SPDX-License-Identifier: Apache-2.0

//! Proc-macro crate for Weaver command boilerplate generation.
//!
//! Provides `#[derive(WeaverCommand)]` which, given an annotated `*Args`
//! struct, generates:
//! - A `XxxConfig` struct with `#[derive(Debug, Clone, Deserialize, JsonSchema)]`
//! - `impl Default for XxxConfig`
//! - `impl weaver_config::CliOverrides for *Args`

mod weaver_command;

use proc_macro::TokenStream;

/// Derive macro that generates `XxxConfig`, its `Default` impl, and a full
/// `impl weaver_config::CliOverrides` block from field annotations.
///
/// # Struct attribute
/// `#[weaver_command(section = "name")]` — required. Sets `SUBCOMMAND`,
/// Config struct name (kebab/snake → PascalCase + "Config"), and TOML key.
/// Add `no_policy` to also emit `fn uses_policy() -> bool { false }`.
///
/// # Field attributes
/// - `#[shared(registry|policy|diagnostic)]` — generates the corresponding
///   `apply_*_overrides` method and includes the field type's `EXCLUDED_ARGS`.
/// - `#[config(default = "value")]` — Config field `T` (from `Option<T>`) with
///   the given default; generates `override_if_set!(config.f, self.f)`.
/// - `#[config]` — Config field `Option<T>` with default `None`; generates
///   `override_if_set!(config.f, self.f, optional)`.
/// - `#[config_only]` / `#[config_only(default = "value")]` — same as `#[config]`
///   / `#[config(default = ...)]` but adds the field to `config_only_fields()`.
/// - *(no annotation)* — CLI-only; field name auto-added to `excluded_args()`.
#[proc_macro_derive(WeaverCommand, attributes(weaver_command, shared, config, config_only))]
pub fn derive_weaver_command(input: TokenStream) -> TokenStream {
    weaver_command::derive(input)
}
