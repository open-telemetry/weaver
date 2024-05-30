// SPDX-License-Identifier: Apache-2.0

//! Generate the JSON Schema of the resolved telemetry schema.

use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use miette::Diagnostic;
use schemars::schema_for;
use serde::Serialize;
use serde_json::to_string_pretty;
use std::path::PathBuf;
use weaver_cache::Cache;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::Logger;
use weaver_resolved_schema::ResolvedTelemetrySchema;

/// Parameters for the `schema json-schema` sub-command
#[derive(Debug, Args)]
pub struct SchemaJsonSchemaArgs {
    /// Output file to write the JSON schema to
    /// If not specified, the JSON schema is printed to stdout
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// An error that can occur while generating a JSON Schema.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Serialize, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    /// The serialization of the JSON schema failed.
    #[error("The serialization of the JSON schema failed. Error: {error}")]
    SerializationError {
        /// The error that occurred.
        error: String,
    },

    /// Writing to the file failed.
    #[error("Writing to the file ‘{file}’ failed for the following reason: {error}")]
    WriteError {
        /// The path to the output file.
        file: PathBuf,
        /// The error that occurred.
        error: String,
    },
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// Generate the JSON Schema of a Telemetry Schema and write the JSON schema to a
/// file or print it to stdout.
#[cfg(not(tarpaulin_include))]
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    _cache: &Cache,
    args: &SchemaJsonSchemaArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let json_schema = schema_for!(ResolvedTelemetrySchema);

    let json_schema_str =
        to_string_pretty(&json_schema).map_err(|e| Error::SerializationError {
            error: e.to_string(),
        })?;

    if let Some(output) = &args.output {
        logger.loading(&format!("Writing JSON schema to `{}`", output.display()));
        std::fs::write(output, json_schema_str).map_err(|e| Error::WriteError {
            file: output.clone(),
            error: e.to_string(),
        })?;
    } else {
        logger.log(&json_schema_str);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: args.output.is_none(),
    })
}

#[cfg(test)]
mod tests {
    use weaver_common::in_memory;
    use weaver_common::in_memory::LogMessage;

    use crate::cli::{Cli, Commands};
    use crate::run_command;
    use crate::schema::json_schema::SchemaJsonSchemaArgs;
    use crate::schema::{SchemaCommand, SchemaSubCommand};

    #[test]
    fn test_registry_json_schema() {
        let logger = in_memory::Logger::new(0);
        let cli = Cli {
            debug: 0,
            quiet: false,
            command: Some(Commands::Schema(SchemaCommand {
                command: SchemaSubCommand::JsonSchema(SchemaJsonSchemaArgs {
                    output: None,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);

        // We should have a single log message with the JSON schema.
        let messages = logger.messages();
        assert_eq!(messages.len(), 1);

        let message = &messages[0];
        if let LogMessage::Log(log) = message {
            let value =
                serde_json::from_str::<serde_json::Value>(log).expect("Failed to parse JSON");
            let definitions = value
                .as_object()
                .expect("Expected a JSON object")
                .get("definitions");
            assert!(
                definitions.is_some(),
                "Expected a 'definitions' key in the JSON schema"
            );
        } else {
            panic!("Expected a log message, but got: {:?}", message);
        }
    }
}
