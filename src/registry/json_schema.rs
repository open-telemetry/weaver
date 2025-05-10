// SPDX-License-Identifier: Apache-2.0

//! Generate the JSON Schema of the resolved registry documents consumed by the template generator
//! and the policy engine.

use crate::ExitDirectives;
use log::info;
use miette::Diagnostic;
use schemars::schema_for;
use serde::Serialize;
use serde_json::to_string_pretty;
use std::{io::Write, path::PathBuf};
pub(crate) use weaver_cli::registry::json_schema::RegistryJsonSchemaArgs;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_forge::registry::ResolvedRegistry;

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

/// Generate the JSON Schema of a ResolvedRegistry and write the JSON schema to a
/// file or print it to stdout.
pub(crate) fn command(args: &RegistryJsonSchemaArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    let json_schema = schema_for!(ResolvedRegistry);

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

    use crate::registry::{RegistryCommand, RegistrySubCommand};
    use crate::run_command;
    use std::fs;
    use tempfile::NamedTempFile;
    use weaver_cli::cli::{Cli, Commands};
    use weaver_cli::registry::json_schema::RegistryJsonSchemaArgs;

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
