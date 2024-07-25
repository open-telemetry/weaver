// SPDX-License-Identifier: Apache-2.0

//! Initializes a `diagnostic_templates` directory to define or override diagnostic output formats.

use crate::diagnostic::{Error, DEFAULT_DIAGNOSTIC_TEMPLATES};
use crate::{DiagnosticArgs, ExitDirectives};
use clap::Args;
use include_dir::DirEntry;
use std::fs;
use std::path::{Path, PathBuf};
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;

/// Parameters for the `diagnostic init` sub-command
#[derive(Debug, Args)]
pub struct DiagnosticInitArgs {
    /// Optional target to initialize the diagnostic templates for. If empty, all default templates will be extracted.
    #[arg(default_value = "")]
    pub target: String,

    /// Optional path where the diagnostic templates directory should be created.
    #[arg(short = 't', long, default_value = "diagnostic_templates")]
    pub diagnostic_templates_dir: PathBuf,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Initializes a `diagnostic_templates` directory to define or override diagnostic output formats.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &DiagnosticInitArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    extract(args.diagnostic_templates_dir.clone(), &args.target).map_err(|e| {
        Error::InitDiagnosticError {
            path: args.diagnostic_templates_dir.clone(),
            error: e.to_string(),
        }
    })?;

    logger.success(&format!(
        "Diagnostic templates initialized at {:?}",
        args.diagnostic_templates_dir
    ));
    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}

/// Extracts the diagnostic templates to the specified path for the given target.
/// If the target is empty, all templates will be extracted.
fn extract<S: AsRef<Path>>(base_path: S, target: &str) -> std::io::Result<()> {
    let base_path = base_path.as_ref();

    for entry in DEFAULT_DIAGNOSTIC_TEMPLATES.entries() {
        let path = base_path.join(entry.path());

        match entry {
            DirEntry::Dir(d) => {
                if d.path().starts_with(target) {
                    fs::create_dir_all(&path)?;
                    d.extract(base_path)?;
                }
            }
            DirEntry::File(f) => {
                fs::write(path, f.contents())?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempdir::TempDir;

    use weaver_common::TestLogger;

    use crate::cli::{Cli, Commands};
    use crate::diagnostic::init::DiagnosticInitArgs;
    use crate::diagnostic::{DiagnosticCommand, DiagnosticSubCommand};
    use crate::run_command;

    #[test]
    fn test_diagnostic_init() {
        let logger = TestLogger::new();
        let temp_output = TempDir::new("output")
            .expect("Failed to create temporary directory")
            .into_path();

        // Let's init for all targets
        let cli = Cli {
            debug: 0,
            quiet: false,
            command: Some(Commands::Diagnostic(DiagnosticCommand {
                command: DiagnosticSubCommand::Init(DiagnosticInitArgs {
                    target: "".to_owned(),
                    diagnostic_templates_dir: temp_output.clone(),
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);

        // Check the presence of 3 subdirectories in the temp_output directory
        let subdirs = fs::read_dir(&temp_output).unwrap().count();
        assert_eq!(subdirs, 3);

        // Let's init for a specific target
        let temp_output = TempDir::new("output")
            .expect("Failed to create temporary directory")
            .into_path();
        let cli = Cli {
            debug: 0,
            quiet: false,
            command: Some(Commands::Diagnostic(DiagnosticCommand {
                command: DiagnosticSubCommand::Init(DiagnosticInitArgs {
                    target: "json".to_owned(),
                    diagnostic_templates_dir: temp_output.clone(),
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);

        // Check the presence of 3 subdirectories in the temp_output directory
        let subdirs = fs::read_dir(&temp_output).unwrap().count();
        assert_eq!(subdirs, 1);
    }
}
