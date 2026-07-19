// SPDX-License-Identifier: Apache-2.0

//! Trait and helpers for CLI-to-config override logic.
//!
//! Each command's `*Args` struct implements [`CliOverrides`] to declare how CLI
//! flags map onto its config section. The [`crate::override_if_set!`] macro
//! reduces the per-field override to a single line.

use schemars::JsonSchema;

use crate::effective::{EffectiveDiagnosticConfig, EffectivePolicyConfig, EffectiveRegistryConfig};
use crate::WeaverConfig;

/// The unified result of loading all configuration for a command.
///
/// Returned by load_config() — commands destructure what they need.
pub struct CommandConfig<C> {
    /// Command-specific configuration.
    pub config: C,
    /// Effective registry settings (defaults → config → CLI).
    pub registry: EffectiveRegistryConfig,
    /// Effective policy settings (defaults → config → CLI).
    pub policy: EffectivePolicyConfig,
}

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
/// The field-mapping methods (`config_only_fields`, `excluded_args`,
/// `field_mappings`) enable a generic consistency test that verifies every
/// config field has a CLI arg and vice versa, with no per-command test code.
pub trait CliOverrides {
    /// The config section type (e.g., `LiveCheckConfig`).
    type Config: Default + Clone + JsonSchema;

    /// The clap subcommand name (e.g. `"live-check"`), used for test introspection.
    const SUBCOMMAND: &'static str;

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

    /// Args that should be excluded from the per-command config consistency check.
    ///
    /// Two categories belong here:
    /// - Shared args (registry, policy, diagnostic) — handled by the shared
    ///   `CommandConfig` sections, not the command-specific config `C`.
    /// - Truly CLI-only args (e.g. `param`, `params`) — no config equivalent.
    #[must_use]
    fn excluded_args() -> &'static [&'static str] {
        &[]
    }

    /// Name mappings for fields where the flattened config name differs
    /// from the CLI arg name.
    #[must_use]
    fn field_mappings() -> &'static [FieldMapping] {
        &[]
    }

    /// Apply CLI overrides for registry args onto an effective registry config.
    fn apply_registry_overrides(&self, _config: &mut EffectiveRegistryConfig) {}

    /// Apply CLI overrides for policy args onto an effective policy config.
    fn apply_policy_overrides(&self, _config: &mut EffectivePolicyConfig) {}

    /// Apply CLI overrides for diagnostic args onto an effective diagnostic config.
    fn apply_diagnostic_overrides(&self, _config: &mut EffectiveDiagnosticConfig) {}

    /// Whether this command uses policy args. When `false`, `load_config` supplies
    /// `EffectivePolicyConfig::skip_all()` regardless of config or CLI.
    #[must_use]
    fn uses_policy() -> bool {
        true
    }
}

/// Compose `excluded_args()` from shared `EXCLUDED_ARGS` constants plus
/// command-specific entries, producing a `&'static [&'static str]`.
///
/// Usage:
/// ```ignore
/// fn excluded_args() -> &'static [&'static str] {
///     excluded_args!(
///         RegistryArgs::EXCLUDED_ARGS,
///         PolicyArgs::EXCLUDED_ARGS,
///         DiagnosticArgs::EXCLUDED_ARGS,
///         ["baseline_registry"]
///     )
/// }
/// ```
#[macro_export]
macro_rules! excluded_args {
    ($($slice:expr),+ $(,)?) => {{
        const ITEMS: &[&str] = &{
            $crate::const_concat_slices!($($slice),+)
        };
        ITEMS
    }};
}

/// Helper: concatenate multiple `&[&str]` slices into a single `[&str; N]`
/// at compile time (via `const` evaluation).
#[macro_export]
#[doc(hidden)]
macro_rules! const_concat_slices {
    ($($slice:expr),+ $(,)?) => {{
        // 1. Compute total length at compile time
        const __LEN: usize = 0 $(+ $slice.len())+;
        // 2. Build the array
        const __RESULT: [&str; __LEN] = {
            let mut arr = [""; __LEN];
            let mut i = 0;
            $(
                {
                    let s = $slice;
                    let mut j = 0;
                    while j < s.len() {
                        arr[i] = s[j];
                        i += 1;
                        j += 1;
                    }
                }
            )+
            arr
        };
        __RESULT
    }};
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
