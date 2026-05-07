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

/// Attribute macro that runs **before** `#[derive(Args, WeaverCommand)]`.
///
/// Reads `#[config(default = "val")]` annotations on each field and appends a
/// `[default: val]` doc line so clap includes it in `--help` output. The
/// default value stays defined in exactly one place — the `#[config]` annotation
/// — with no risk of drift.
///
/// Also renames itself to `#[weaver_command_inner(...)]` in its output so the
/// `WeaverCommand` derive can still find the section name without re-triggering
/// this attribute macro.
#[proc_macro_attribute]
pub fn weaver_command(attr: TokenStream, item: TokenStream) -> TokenStream {
    weaver_command::inject_default_docs(attr, item)
}

/// Derive macro that generates `XxxConfig`, its `Default` impl, and a full
/// `impl weaver_config::CliOverrides` block from field annotations.
///
/// # Struct attribute
/// `#[weaver_command(section = "name")]` — required (applied as an attribute
/// macro that runs first; by the time this derive runs the attribute has been
/// renamed to `#[weaver_command_inner(...)]`).
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
#[proc_macro_derive(
    WeaverCommand,
    attributes(weaver_command_inner, shared, config, config_only)
)]
pub fn derive_weaver_command(input: TokenStream) -> TokenStream {
    weaver_command::derive(input)
}
