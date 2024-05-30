// SPDX-License-Identifier: Apache-2.0

//! Generate artifacts for a semantic convention registry.

use std::path::PathBuf;

use clap::Args;

use weaver_cache::Cache;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_forge::file_loader::FileSystemFileLoader;
use weaver_forge::registry::ResolvedRegistry;
use weaver_forge::{OutputDirective, TemplateEngine};
use weaver_semconv::registry::SemConvRegistry;

use crate::registry::RegistryArgs;
use crate::util::{
    check_policies, load_semconv_specs, resolve_semconv_specs, semconv_registry_path_from,
};
use crate::DiagnosticArgs;

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
    pub templates: PathBuf,

    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Optional list of policy files to check against the files of the semantic
    /// convention registry.
    #[arg(short = 'p', long = "policy")]
    pub policies: Vec<PathBuf>,

    /// Skip the policy checks.
    #[arg(long, default_value = "false")]
    pub skip_policies: bool,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Generate artifacts from a semantic convention registry.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistryGenerateArgs,
) -> Result<(), DiagnosticMessages> {
    logger.loading(&format!(
        "Generating artifacts for the registry `{}`",
        args.registry.registry
    ));

    let registry_id = "default";
    let registry_path =
        semconv_registry_path_from(&args.registry.registry, &args.registry.registry_git_sub_dir);

    // Load the semantic convention registry into a local cache.
    let semconv_specs = load_semconv_specs(&registry_path, cache, logger.clone())?;

    if !args.skip_policies {
        check_policies(
            &registry_path,
            cache,
            &args.policies,
            &semconv_specs,
            logger.clone(),
        )?;
    }

    let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);
    let schema = resolve_semconv_specs(&mut registry, logger.clone())?;
    let loader = FileSystemFileLoader::try_new(args.templates.join("registry"), &args.target)?;
    let engine = TemplateEngine::try_new(loader)?;

    let template_registry = ResolvedRegistry::try_from_resolved_registry(
        schema
            .registry(registry_id)
            .expect("Failed to get the registry from the resolved schema"),
        schema.catalog(),
    )?;

    engine.generate(
        logger.clone(),
        &template_registry,
        args.output.as_path(),
        &OutputDirective::File,
    )?;

    logger.success("Artifacts generated successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempdir::TempDir;

    use weaver_common::TestLogger;

    use crate::cli::{Cli, Commands};
    use crate::registry::generate::RegistryGenerateArgs;
    use crate::registry::{RegistryArgs, RegistryCommand, RegistryPath, RegistrySubCommand};
    use crate::run_command;

    #[test]
    fn test_registry_generate() {
        let logger = TestLogger::new();
        let temp_output = TempDir::new("output")
            .expect("Failed to create temporary directory")
            .into_path();
        let cli = Cli {
            debug: 0,
            quiet: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Generate(RegistryGenerateArgs {
                    target: "rust".to_owned(),
                    output: temp_output.clone(),
                    templates: PathBuf::from("crates/weaver_codegen_test/templates/"),
                    registry: RegistryArgs {
                        registry: RegistryPath::Local(
                            "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        ),
                        registry_git_sub_dir: None,
                    },
                    policies: vec![],
                    skip_policies: true,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_code = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_code, 0);

        // Hashset containing recursively all the relative paths of rust files in the
        // output directory.
        let rust_files: std::collections::HashSet<_> = walkdir::WalkDir::new(&temp_output)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
            .map(|e| {
                e.path()
                    .strip_prefix(&temp_output)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        let expected_rust_files = vec![
            "attributes/client.rs",
            "metrics/system.rs",
            "attributes/mod.rs",
            "metrics/http.rs",
            "attributes/exception.rs",
            "attributes/server.rs",
            "metrics/mod.rs",
            "attributes/network.rs",
            "attributes/url.rs",
            "attributes/http.rs",
            "attributes/system.rs",
            "attributes/error.rs",
        ]
        .into_iter()
        .map(|s| {
            // Split the string by `/` and join the parts with the OS specific separator.
            s.split('/')
                .collect::<PathBuf>()
                .to_string_lossy()
                .to_string()
        })
        .collect::<std::collections::HashSet<_>>();

        assert_eq!(rust_files, expected_rust_files);

        // Now, let's run the command again with the policy checks enabled.
        let cli = Cli {
            debug: 0,
            quiet: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Generate(RegistryGenerateArgs {
                    target: "rust".to_owned(),
                    output: temp_output.clone(),
                    templates: PathBuf::from("crates/weaver_codegen_test/templates/"),
                    registry: RegistryArgs {
                        registry: RegistryPath::Local(
                            "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        ),
                        registry_git_sub_dir: None,
                    },
                    policies: vec![],
                    skip_policies: false,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_code = run_command(&cli, logger);
        // The command should exit with an error code.
        assert_eq!(exit_code, 1);
    }
}
