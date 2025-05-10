// SPDX-License-Identifier: Apache-2.0

//! Weaver registry live-check sub-command.

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::DiagnosticArgs;
use clap::Args;
use std::path::PathBuf;

/// The input format
#[derive(Debug, Clone)]
pub enum InputFormat {
    /// Text format
    Text,
    /// JSON format
    Json,
}
impl From<String> for InputFormat {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "json" | "js" => InputFormat::Json,
            _ => InputFormat::Text,
        }
    }
}

/// The input source
#[derive(Debug, Clone)]
pub enum InputSource {
    /// File path
    File(PathBuf),
    /// Standard input
    Stdin,
    /// OpenTelemetry Protocol (OTLP)
    Otlp,
}

impl From<String> for InputSource {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "stdin" | "s" => InputSource::Stdin,
            "otlp" | "o" => InputSource::Otlp,
            _ => InputSource::File(PathBuf::from(s)),
        }
    }
}

/// Parameters for the `registry live-check` sub-command
#[derive(Debug, Args)]
pub struct RegistryLiveCheckArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    pub registry: RegistryArgs,

    /// Policy parameters
    #[command(flatten)]
    pub policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,

    /// Where to read the input telemetry from. Possible values: `{file path}`, `stdin`, `otlp`
    #[arg(long, default_value = "otlp")]
    pub input_source: InputSource,

    /// The format of the input telemetry. (Not required for OTLP). Predefined formats are: `text`, or `json`
    #[arg(long, default_value = "json")]
    pub input_format: InputFormat,

    /// Format used to render the report. Predefined formats are: `ansi`, `json`
    #[arg(long, default_value = "ansi")]
    pub format: String,

    /// Path to the directory where the templates are located.
    #[arg(long, default_value = "live_check_templates")]
    pub templates: PathBuf,

    /// Disable stream mode. Use this flag to disable streaming output.
    ///
    /// When the output is STDOUT, Ingesters that support streaming (STDIN and OTLP),
    /// by default output the live check results for each entity as they are ingested.
    #[arg(long, default_value = "false")]
    pub no_stream: bool,

    /// Path to the directory where the generated artifacts will be saved.
    /// If not specified, the report is printed to stdout.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Address used by the gRPC OTLP listener.
    #[clap(long, default_value = "0.0.0.0")]
    pub otlp_grpc_address: String,

    /// Port used by the gRPC OTLP listener.
    #[clap(long, default_value = "4317")]
    pub otlp_grpc_port: u16,

    /// Port used by the HTTP admin port (endpoints: `/stop`).
    #[clap(long, default_value = "4320")]
    pub admin_port: u16,

    /// Max inactivity time in seconds before stopping the listener.
    #[clap(long, default_value = "10")]
    pub inactivity_timeout: u64,

    /// Advice policies directory. Set this to override the default policies.
    #[arg(long)]
    pub advice_policies: Option<PathBuf>,

    /// Advice preprocessor. A jq script to preprocess the registry data before passing to rego.
    ///
    /// Rego policies are run for each sample as it arrives in a stream. The preprocessor
    /// can be used to create a new data structure that is more efficient for the rego policies
    /// versus processing the data for every sample.
    #[arg(long)]
    pub advice_preprocessor: Option<PathBuf>,
}
