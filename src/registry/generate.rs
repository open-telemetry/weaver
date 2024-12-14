// SPDX-License-Identifier: Apache-2.0

//! Generate artifacts for a semantic convention registry.

use std::path::PathBuf;

use clap::Args;
use miette::Diagnostic;
use serde_yaml::Value;

use weaver_cache::RegistryRepo;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_forge::config::{Params, WeaverConfig};
use weaver_forge::file_loader::{FileLoader, FileSystemFileLoader};
use weaver_forge::registry::ResolvedRegistry;
use weaver_forge::{OutputDirective, TemplateEngine};
use weaver_semconv::registry::SemConvRegistry;

use crate::registry::{CommonRegistryArgs, Error, RegistryArgs};
use crate::util::{check_policy, init_policy_engine, load_semconv_specs, resolve_semconv_specs};
use crate::{registry, DiagnosticArgs, ExitDirectives};

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

    /// List of `weaver.yaml` configuration files to use. When there is a conflict, the last one
    /// will override the previous ones for the keys that are defined in both.
    #[arg(short = 'c', long)]
    pub config: Option<Vec<PathBuf>>,

    /// Parameters key=value, defined in the command line, to pass to the templates.
    /// The value must be a valid YAML value.
    #[arg(short = 'D', long, value_parser = parse_key_val)]
    pub param: Option<Vec<(String, Value)>>,

    /// Parameters, defined in a YAML file, to pass to the templates.
    #[arg(long)]
    pub params: Option<PathBuf>,

    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Optional list of policy files or directories to check against the files of the semantic
    /// convention registry. If a directory is provided all `.rego` files in the directory will be
    /// loaded.
    #[arg(short = 'p', long = "policy")]
    pub policies: Vec<PathBuf>,

    /// Skip the policy checks.
    #[arg(long, default_value = "false")]
    pub skip_policies: bool,

    /// Enable the most recent validation rules for the semconv registry. It is recommended
    /// to enable this flag when checking a new registry.
    #[arg(long, default_value = "false")]
    pub future: bool,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,

    /// Common weaver registry parameters
    #[command(flatten)]
    pub common_registry_args: CommonRegistryArgs,
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
    args: &RegistryGenerateArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    logger.loading(&format!(
        "Generating artifacts for the registry `{}`",
        args.registry.registry
    ));

    let mut diag_msgs = DiagnosticMessages::empty();
    let params = generate_params(args)?;
    let mut registry_path = args.registry.registry.clone();
    // Support for --registry-git-sub-dir (should be removed in the future)
    if let registry::RegistryPath::GitRepo { sub_folder, .. } = &mut registry_path {
        if sub_folder.is_none() {
            sub_folder.clone_from(&args.registry.registry_git_sub_dir);
        }
    }
    let registry_id = "default";
    let registry_repo = RegistryRepo::try_new("main", &registry_path)?;

    // Load the semantic convention registry into a local cache.
    let semconv_specs = load_semconv_specs(
        &registry_repo,
        logger.clone(),
        args.common_registry_args.follow_symlinks,
    )
    .ignore(|e| matches!(e.severity(), Some(miette::Severity::Warning)))
    .into_result_failing_non_fatal()?;

    if !args.skip_policies {
        let policy_engine = init_policy_engine(&registry_repo, &args.policies, false)?;
        check_policy(&policy_engine, &semconv_specs)
            .inspect(|_, violations| {
                if let Some(violations) = violations {
                    logger.success(&format!(
                        "All `before_resolution` policies checked ({} violations found)",
                        violations.len()
                    ));
                } else {
                    logger.success("No `before_resolution` policy violation");
                }
            })
            .capture_non_fatal_errors(&mut diag_msgs)?;
    }

    let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);
    let schema = resolve_semconv_specs(&mut registry, logger.clone())?;
    let loader = FileSystemFileLoader::try_new(args.templates.join("registry"), &args.target)?;
    let config = if let Some(paths) = &args.config {
        WeaverConfig::try_from_config_files(paths)
    } else {
        WeaverConfig::try_from_path(loader.root())
    }?;
    let engine = TemplateEngine::new(config, loader, params);

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

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

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
    use crate::registry::{
        CommonRegistryArgs, RegistryArgs, RegistryCommand, RegistryPath, RegistrySubCommand,
    };
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
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Generate(RegistryGenerateArgs {
                    target: "rust".to_owned(),
                    output: temp_output.clone(),
                    templates: PathBuf::from("crates/weaver_codegen_test/templates/"),
                    config: None,
                    param: None,
                    params: None,
                    registry: RegistryArgs {
                        registry: RegistryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        registry_git_sub_dir: None,
                    },
                    policies: vec![],
                    skip_policies: true,
                    future: false,
                    diagnostic: Default::default(),
                    common_registry_args: CommonRegistryArgs {
                        follow_symlinks: false,
                    },
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
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Generate(RegistryGenerateArgs {
                    target: "rust".to_owned(),
                    output: temp_output.clone(),
                    templates: PathBuf::from("crates/weaver_codegen_test/templates/"),
                    config: None,
                    param: None,
                    params: None,
                    registry: RegistryArgs {
                        registry: RegistryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        registry_git_sub_dir: None,
                    },
                    policies: vec![],
                    skip_policies: false,
                    future: false,
                    diagnostic: Default::default(),
                    common_registry_args: CommonRegistryArgs {
                        follow_symlinks: false,
                    },
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger);
        // The command should exit with an error code.
        assert_eq!(exit_directive.exit_code, 1);
    }

    #[test]
    fn test_registry_generate_with_config() {
        let logger = TestLogger::new();
        let temp_output = TempDir::new("output")
            .expect("Failed to create temporary directory")
            .into_path();
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Generate(RegistryGenerateArgs {
                    target: "rust".to_owned(),
                    output: temp_output.clone(),
                    templates: PathBuf::from("crates/weaver_codegen_test/templates/"),
                    config: Some(vec![
                        PathBuf::from(
                            "crates/weaver_codegen_test/templates/registry/alt_weaver.yaml",
                        ),
                        PathBuf::from(
                            "crates/weaver_codegen_test/templates/registry/rust/weaver.yaml",
                        ),
                    ]),
                    param: None,
                    params: None,
                    registry: RegistryArgs {
                        registry: RegistryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        registry_git_sub_dir: None,
                    },
                    policies: vec![],
                    skip_policies: true,
                    future: false,
                    diagnostic: Default::default(),
                    common_registry_args: CommonRegistryArgs {
                        follow_symlinks: false,
                    },
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
            "attributes/mod.rs",
            "attributes/exception.rs",
            "attributes/server.rs",
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
    }

    #[test]
    fn test_registry_generate_with_symbolic_link_cases() {
        let test_cases = vec![
            (
                true, // follow_symlinks
                vec![
                    // expected files when following symlinks
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
                ],
            ),
            (
                false,  // don't follow_symlinks
                vec![], // expect no files when not following symlinks
            ),
        ];

        for (follow_symlinks, expected_files) in test_cases {
            let logger = TestLogger::new();
            let temp_output = TempDir::new("output")
                .expect("Failed to create temporary directory")
                .into_path();

            let cli = Cli {
                debug: 0,
                quiet: false,
                future: false,
                command: Some(Commands::Registry(RegistryCommand {
                    command: RegistrySubCommand::Generate(RegistryGenerateArgs {
                        target: "rust".to_owned(),
                        output: temp_output.clone(),
                        templates: PathBuf::from("crates/weaver_codegen_test/templates/"),
                        config: None,
                        param: None,
                        params: None,
                        registry: RegistryArgs {
                            registry: RegistryPath::LocalFolder {
                                path: "data/symbolic_test/".to_owned(),
                            },
                            registry_git_sub_dir: None,
                        },
                        policies: vec![],
                        skip_policies: true,
                        future: false,
                        diagnostic: Default::default(),
                        common_registry_args: CommonRegistryArgs { follow_symlinks },
                    }),
                })),
            };

            let exit_directive = run_command(&cli, logger.clone());
            // The command should succeed in both cases
            assert_eq!(exit_directive.exit_code, 0);

            // Get the actual generated files
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

            // Convert expected files to paths with proper OS separators
            let expected_rust_files: std::collections::HashSet<_> = expected_files
                .into_iter()
                .map(|s| {
                    s.split('/')
                        .collect::<PathBuf>()
                        .to_string_lossy()
                        .to_string()
                })
                .collect();

            assert_eq!(
                rust_files, expected_rust_files,
                "File sets don't match for follow_symlinks = {}",
                follow_symlinks
            );
        }
    }
}
