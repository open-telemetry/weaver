// SPDX-License-Identifier: Apache-2.0

//! Weaver registry generate sub-command.

use crate::registry::{Error, PolicyArgs, RegistryArgs};
use crate::DiagnosticArgs;
use clap::Args;
use serde_yaml::Value;
use std::path::PathBuf;
use weaver_common::vdir::VirtualDirectoryPath;

/// Parameters for the `registry generate` sub-command
#[derive(Debug, Args)]
pub struct RegistryGenerateArgs {
    /// Target to generate the artifacts for.
    pub target: String,

    /// Path to the directory where the generated artifacts will be saved.
    /// Default is the `output` directory.
    #[arg(default_value = "output")]
    pub output: PathBuf,

    /// Path to the directory where the templates are located.
    /// Default is the `templates` directory.
    #[arg(short = 't', long, default_value = "templates")]
    pub templates: VirtualDirectoryPath,

    /// List of `weaver.yaml` configuration files to use. When there is a conflict, the last one
    /// will override the previous ones for the keys that are defined in both.
    #[arg(short = 'c', long)]
    pub config: Option<Vec<PathBuf>>,

    /// Parameters `key=value`, defined in the command line, to pass to the templates.
    /// The value must be a valid YAML value.
    #[arg(short = 'D', long, value_parser = parse_key_val)]
    pub param: Option<Vec<(String, Value)>>,

    /// Parameters, defined in a YAML file, to pass to the templates.
    #[arg(long)]
    pub params: Option<PathBuf>,

    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    pub registry: RegistryArgs,

    /// Policy parameters
    #[command(flatten)]
    pub policy: PolicyArgs,

    /// Enable the most recent validation rules for the semconv registry. It is recommended
    /// to enable this flag when checking a new registry.
    #[arg(long, default_value = "false")]
    pub future: bool,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Utility function to parse key-value pairs from the command line.
fn parse_key_val(s: &str) -> Result<(String, Value), Error> {
    let pos = s.find('=').ok_or_else(|| Error::InvalidParam {
        param: s.to_owned(),
        error: "A valid parameter definition is `--param <name>=<yaml-value>`".to_owned(),
    })?;
    let value = serde_yaml::from_str(&s[pos + 1..]).map_err(|e| Error::InvalidParam {
        param: s.to_owned(),
        error: format!(
            "A valid parameter definition is `--param <name>=<yaml-value>`. Error: {}",
            e
        ),
    })?;
    Ok((s[..pos].to_string(), value))
}
