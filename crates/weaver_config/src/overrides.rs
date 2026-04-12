// SPDX-License-Identifier: Apache-2.0

//! Trait and helpers for CLI-to-config override logic.
//!
//! Each command's `*Args` struct implements [`CliOverrides`] to declare how CLI
//! flags map onto its config section. The [`crate::override_if_set!`] macro
//! reduces the per-field override to a single line.

use schemars::JsonSchema;
use std::path::PathBuf;

use crate::WeaverConfig;

/// A name mapping between a config field and its CLI arg counterpart.
///
/// Used when TOML nesting produces a different flattened name than the CLI arg.
/// For example, config field `otlp.admin_port` flattens to `otlp_admin_port`
/// but the CLI arg is just `admin_port`.
pub struct FieldMapping {
    /// The config field name (flattened from the JSON schema, e.g. `otlp_admin_port`).
    pub config_name: &'static str,
    /// The CLI arg name (from clap, underscored, e.g. `admin_port`).
    pub cli_name: &'static str,
}

/// Trait for CLI args structs that can load and override a config section.
///
/// Implement this on each command's `*Args` struct to enable the generic
/// `load_config()` flow: defaults → `.weaver.toml` → CLI overrides.
///
/// The field-mapping methods (`config_only_fields`, `cli_only_args`,
/// `field_mappings`) enable a generic consistency test that verifies every
/// config field has a CLI arg and vice versa, with no per-command test code.
pub trait CliOverrides {
    /// The config section type (e.g., `LiveCheckConfig`).
    type Config: Default + Clone + JsonSchema;

    /// The clap subcommand name (e.g. `"live-check"`), used for test introspection.
    const SUBCOMMAND: &'static str;

    /// Path to a `--config` flag, if the command supports one.
    fn config_path(&self) -> Option<&PathBuf>;

    /// Extract the relevant section from a loaded `WeaverConfig`.
    fn extract_config(weaver_config: &WeaverConfig) -> Self::Config;

    /// Apply CLI arg overrides onto the config. Only `Some` values overwrite.
    fn apply_overrides(&self, config: &mut Self::Config);

    /// Config fields that intentionally have no CLI counterpart.
    /// Each entry should include a reason comment in the impl.
    #[must_use]
    fn config_only_fields() -> &'static [&'static str] {
        &[]
    }

    /// CLI args that intentionally have no config counterpart.
    /// Includes args from flattened sub-structs (registry, policy, diagnostic)
    /// and any args that control config loading itself.
    #[must_use]
    fn cli_only_args() -> &'static [&'static str] {
        &[]
    }

    /// Name mappings for fields where the flattened config name differs
    /// from the CLI arg name.
    #[must_use]
    fn field_mappings() -> &'static [FieldMapping] {
        &[]
    }
}

/// Apply a single CLI override: if `src` is `Some`, write it to `dst`.
///
/// Two forms:
/// - `override_if_set!(dst, src)` — for `Option<T> → T` (clone into existing value)
/// - `override_if_set!(dst, src, optional)` — for `Option<T> → Option<T>` (wrap in `Some`)
#[macro_export]
macro_rules! override_if_set {
    ($dst:expr, $src:expr) => {
        if let Some(v) = &$src {
            $dst.clone_from(v);
        }
    };
    ($dst:expr, $src:expr, optional) => {
        if let Some(v) = &$src {
            $dst = Some(v.clone());
        }
    };
}
