// SPDX-License-Identifier: Apache-2.0

//! Generate artifacts for a semantic convention registry.

use std::path::PathBuf;

use clap::Args;
use serde_yaml::Value;

use weaver_cache::Cache;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_forge::config::Params;
use weaver_forge::file_loader::FileSystemFileLoader;
use weaver_forge::registry::ResolvedRegistry;
use weaver_forge::{OutputDirective, TemplateEngine};
use weaver_semconv::registry::SemConvRegistry;

use crate::registry::{Error, RegistryArgs};
use crate::util::{
    check_policies, init_policy_engine, load_semconv_specs, resolve_semconv_specs,
    semconv_registry_path_from,
};
use crate::{DiagnosticArgs, ExitDirectives};

/// Parameters for the `registry generate` sub-command
#[derive(Debug, Args)]
pub struct RegistryGenerateArgs {
    /// Path to the directory where the generated artifacts will be saved.
    /// Default is the `output` directory.
    #[arg(default_value = "output")]
    pub output: PathBuf,

    /// Path to the directory where the templates are located.
    /// Default is the `templates` directory.
    #[arg(short = 't', long, default_value = "templates")]
    pub templates: PathBuf,

    /// Parameters key=value, defined in the command line, to pass to the templates.
    /// The value must be a valid YAML value.
    #[arg(short= 'D', long, value_parser = parse_key_val)]
    pub param: Option<Vec<(String, Value)>>,

    /// Parameters, defined in a YAML file, to pass to the templates.
    #[arg(long)]
    pub params: Option<PathBuf>,

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

/// Generate artifacts from a semantic convention registry.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistryGenerateArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    logger.loading(&format!(
        "Generating artifacts for the registry `{}`",
        args.registry.registry
    ));

    let params = generate_params(args)?;
    let registry_id = "default";
    let registry_path =
        semconv_registry_path_from(&args.registry.registry, &args.registry.registry_git_sub_dir);

    // Load the semantic convention registry into a local cache.
    let semconv_specs = load_semconv_specs(&registry_path, cache, logger.clone())?;

    if !args.skip_policies {
        let policy_engine = init_policy_engine(&registry_path, cache, &args.policies, false)?;
        check_policies(&policy_engine, &semconv_specs, logger.clone())?;
    }

    let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);
    let schema = resolve_semconv_specs(&mut registry, logger.clone())?;
    let loader = FileSystemFileLoader::try_new(args.templates.clone())?;
    let engine = TemplateEngine::try_new(loader, params)?;

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
    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}

/// Generate the parameters to pass to the templates.
/// The `--params` argument (if provided) is used to load the parameters from a YAML file.
/// Then the key-value pairs from the `--param` arguments are added to the parameters.
/// So `--param key=value` will override the value of `key` if it exists in the YAML file.
fn generate_params(args: &RegistryGenerateArgs) -> Result<Params, Error> {
    // Load the parameters from the YAML file or if not provided, use the default parameters.
    let mut params = if let Some(params_file) = &args.params {
        let file = std::fs::File::open(params_file).map_err(|e| Error::InvalidParams {
            params_file: params_file.clone(),
            error: e.to_string(),
        })?;
        serde_yaml::from_reader(file).map_err(|e| Error::InvalidParams {
            params_file: params_file.clone(),
            error: e.to_string(),
        })?
    } else {
        Params::default()
    };

    // Override the parameters with the key-value pairs from the command line.
    if let Some(param) = &args.param {
        for (name, value) in param {
            _ = params.params.insert(name.clone(), value.clone());
        }
    }

    Ok(params)
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
                    output: temp_output.clone(),
                    templates: PathBuf::from("crates/weaver_codegen_test/templates/registry/rust"),
                    param: None,
                    params: None,
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

        let exit_directive = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);

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
                    output: temp_output.clone(),
                    templates: PathBuf::from("crates/weaver_codegen_test/templates/registry/rust"),
                    param: None,
                    params: None,
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

        let exit_directive = run_command(&cli, logger);
        // The command should exit with an error code.
        assert_eq!(exit_directive.exit_code, 1);
    }
}
