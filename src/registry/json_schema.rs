// SPDX-License-Identifier: Apache-2.0

//! Generate the JSON Schema of the resolved registry documents consumed by the template generator
//! and the policy engine.

use crate::{DiagnosticArgs, ExitDirectives};
use clap::{Args, ValueEnum};
use log::info;
use miette::Diagnostic;
use schemars::schema_for;
use serde::Serialize;
use serde_json::to_string_pretty;
use std::{io::Write, path::PathBuf};
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_forge::registry::ResolvedRegistry;
use weaver_semconv::semconv::SemConvSpec;

/// Parameters for the `registry json-schema` sub-command
#[derive(Debug, Args)]
pub struct RegistryJsonSchemaArgs {
    /// The type of JSON schema to generate
    #[arg(short, long, value_enum, default_value_t = JsonSchemaType::ResolvedRegistry)]
    json_schema: JsonSchemaType,

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

/// The type of JSON schema to generate.
#[derive(Debug, Clone, ValueEnum)]
pub enum JsonSchemaType {
    /// The JSON schema of a resolved registry.
    ResolvedRegistry,
    /// The JSON schema of a semantic convention group.
    SemconvGroup,
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(vec![DiagnosticMessage::new(error)])
    }
}

/// Generate the JSON Schema of a ResolvedRegistry and write the JSON schema to a
/// file or print it to stdout.
pub(crate) fn command(args: &RegistryJsonSchemaArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    let json_schema = match args.json_schema {
        JsonSchemaType::ResolvedRegistry => schema_for!(ResolvedRegistry),
        JsonSchemaType::SemconvGroup => schema_for!(SemConvSpec)
    };

    let json_schema_str =
        to_string_pretty(&json_schema).map_err(|e| Error::SerializationError {
            error: e.to_string(),
        })?;

    if let Some(output) = &args.output {
        info!("Writing JSON schema to `{}`", output.display());
        std::fs::write(output, json_schema_str).map_err(|e| Error::WriteError {
            file: output.clone(),
            error: e.to_string(),
        })?;
    } else {
        std::io::stdout()
            .write_all(json_schema_str.as_bytes())
            .map_err(|e| Error::WriteError {
                file: PathBuf::from("stdout"),
                error: e.to_string(),
            })?;
    }

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}

#[cfg(test)]
mod tests {

    use crate::cli::{Cli, Commands};
    use crate::registry::json_schema::{JsonSchemaType, RegistryJsonSchemaArgs};
    use crate::registry::{RegistryCommand, RegistrySubCommand};
    use crate::run_command;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_registry_json_schema() {
        // Create a temporary file for the output
        let temp_file = NamedTempFile::new().expect("Failed to create temporary file");
        let output_path = temp_file.path().to_path_buf();

        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::JsonSchema(RegistryJsonSchemaArgs {
                    json_schema: JsonSchemaType::ResolvedRegistry,
                    output: Some(output_path.clone()),
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);

        // Read the content of the temp file
        let json_content = fs::read_to_string(output_path).expect("Failed to read temporary file");

        // Parse and validate the JSON content
        let value =
            serde_json::from_str::<serde_json::Value>(&json_content).expect("Failed to parse JSON");

        let definitions = value
            .as_object()
            .expect("Expected a JSON object")
            .get("definitions");

        assert!(
            definitions.is_some(),
            "Expected a 'definitions' key in the JSON schema"
        );
    }
}
