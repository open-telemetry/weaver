// SPDX-License-Identifier: Apache-2.0

//! Generate artifacts for a semantic convention registry.

use std::path::PathBuf;

use clap::Args;
use log::info;
use serde_yaml::Value;

use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::log_success;
use weaver_forge::config::{Params, WeaverConfig};
use weaver_forge::file_loader::{FileLoader, FileSystemFileLoader};
use weaver_forge::{OutputDirective, TemplateEngine};

use crate::registry::{Error, PolicyArgs, RegistryArgs};
use crate::weaver::{ResolvedV2, WeaverEngine};
use crate::{DiagnosticArgs, ExitDirectives};
use weaver_common::vdir::VirtualDirectory;
use weaver_common::vdir::VirtualDirectoryPath;

/// Parameters for the `registry generate` sub-command
#[derive(Debug, Args)]
pub struct RegistryGenerateArgs {
    /// Target to generate the artifacts for.
    #[arg(default_value = "")]
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

    /// Policy parameters
    #[command(flatten)]
    policy: PolicyArgs,

    /// Enable the most recent validation rules for the semconv registry. It is recommended
    /// to enable this flag when checking a new registry.
    #[arg(long, default_value = "false")]
    pub future: bool,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Utility function to parse key-value pairs from the command line.
pub(crate) fn parse_key_val(s: &str) -> Result<(String, Value), Error> {
    let pos = s.find('=').ok_or_else(|| Error::InvalidParam {
        param: s.to_owned(),
        error: "A valid parameter definition is `--param <name>=<yaml-value>`".to_owned(),
    })?;
    let value = serde_yaml::from_str(&s[pos + 1..]).map_err(|e| Error::InvalidParam {
        param: s.to_owned(),
        error: format!("A valid parameter definition is `--param <name>=<yaml-value>`. Error: {e}"),
    })?;
    Ok((s[..pos].to_string(), value))
}

/// Generate artifacts from a semantic convention registry.
pub(crate) fn command(args: &RegistryGenerateArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    info!(
        "Generating artifacts for the registry `{}`",
        args.registry.registry
    );

    let mut diag_msgs = DiagnosticMessages::empty();
    let weaver = WeaverEngine::new(&args.registry, &args.policy);
    let resolved = weaver.load_and_resolve_main(&mut diag_msgs)?;
    let params = generate_params(args)?;
    let templates_dir =
        VirtualDirectory::try_new(&args.templates).map_err(|e| Error::InvalidParams {
            params_file: PathBuf::from(args.templates.to_string()),
            error: e.to_string(),
        })?;
    let loader =
        FileSystemFileLoader::try_new(resolve_templates_root(&templates_dir), &args.target)?;
    let config = if let Some(paths) = &args.config {
        WeaverConfig::try_from_config_files(paths)
    } else {
        WeaverConfig::try_from_path(loader.root())
    }?;
    let engine = TemplateEngine::try_new(config, loader, params)?;
    // Resolve v1 and v2 schema, based on user request.
    if args.registry.v2 {
        let resolved_v2: ResolvedV2 = resolved.try_into()?;
        resolved_v2.check_after_resolution_policy(&mut diag_msgs)?;
        engine.generate(
            &resolved_v2.template_schema(),
            args.output.as_path(),
            &OutputDirective::File,
        )?;
    } else {
        resolved.check_after_resolution_policy(&mut diag_msgs)?;
        engine.generate(
            &resolved.template_schema(),
            args.output.as_path(),
            &OutputDirective::File,
        )?;
    }

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    log_success("Artifacts generated successfully");
    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}

/// Resolve the effective templates root.
/// If a `registry` subdirectory exists under the provided templates directory,
/// that subdirectory is returned, otherwise the original directory path is returned.
fn resolve_templates_root(templates_dir: &VirtualDirectory) -> PathBuf {
    let base = templates_dir.path();
    let candidate = base.join("registry");
    if candidate.is_dir() {
        candidate
    } else {
        base.to_path_buf()
    }
}

/// Generate the parameters to pass to the templates.
/// The `--params` argument (if provided) is used to load the parameters from a YAML file.
/// Then the key-value pairs from the `--param` arguments are added to the parameters.
/// So `--param key=value` will override the value of `key` if it exists in the YAML file.
fn generate_params(args: &RegistryGenerateArgs) -> Result<Params, Error> {
    generate_params_shared(&args.param, &args.params)
}

pub(crate) fn generate_params_shared(
    direct: &Option<Vec<(String, Value)>>,
    file: &Option<PathBuf>,
) -> Result<Params, Error> {
    // Load the parameters from the YAML file or if not provided, use the default parameters.
    let mut params = if let Some(params_file) = file {
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
    if let Some(param) = direct {
        for (name, value) in param {
            _ = params.params.insert(name.clone(), value.clone());
        }
    }
    Ok(params)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;
    use weaver_diff::diff_dir;

    use crate::cli::{Cli, Commands};
    use crate::registry::generate::RegistryGenerateArgs;
    use crate::registry::{PolicyArgs, RegistryArgs, RegistryCommand, RegistrySubCommand};
    use crate::run_command;
    use weaver_common::vdir::VirtualDirectoryPath;

    #[test]
    fn test_registry_generate() {
        let temp_output = tempfile::Builder::new()
            .prefix("output")
            .tempdir()
            .expect("Failed to create temporary directory")
            .keep();
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Generate(RegistryGenerateArgs {
                    target: "rust".to_owned(),
                    output: temp_output.clone(),
                    templates: VirtualDirectoryPath::LocalFolder {
                        path: "crates/weaver_codegen_test/templates/".to_owned(),
                    },
                    config: None,
                    param: None,
                    params: None,
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        v2: false,
                    },
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: true,
                        display_policy_coverage: false,
                    },
                    future: false,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);

        // Hashset containing recursively all the relative paths of rust files in the
        // output directory.
        let rust_files: std::collections::HashSet<_> = walkdir::WalkDir::new(&temp_output)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
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
                    templates: VirtualDirectoryPath::LocalFolder {
                        path: "crates/weaver_codegen_test/templates/".to_owned(),
                    },
                    config: None,
                    param: None,
                    params: None,
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        v2: false,
                    },
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: false,
                        display_policy_coverage: false,
                    },
                    future: false,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should exit with an error code.
        assert_eq!(exit_directive.exit_code, 1);
    }

    #[test]
    fn test_registry_generate_with_config() {
        let temp_output = TempDir::new()
            .expect("Failed to create temporary directory")
            .keep();
        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Generate(RegistryGenerateArgs {
                    target: "rust".to_owned(),
                    output: temp_output.clone(),
                    templates: VirtualDirectoryPath::LocalFolder {
                        path: "crates/weaver_codegen_test/templates/".to_owned(),
                    },
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
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        v2: false,
                    },
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: true,
                        display_policy_coverage: false,
                    },
                    future: false,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);

        // Hashset containing recursively all the relative paths of rust files in the
        // output directory.
        let rust_files: std::collections::HashSet<_> = walkdir::WalkDir::new(&temp_output)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
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
        env_logger::builder().is_test(true).init();
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
            let temp_output = TempDir::new()
                .expect("Failed to create temporary directory")
                .keep();

            let cli = Cli {
                debug: 1,
                quiet: false,
                future: false,
                command: Some(Commands::Registry(RegistryCommand {
                    command: RegistrySubCommand::Generate(RegistryGenerateArgs {
                        target: "rust".to_owned(),
                        output: temp_output.clone(),
                        templates: VirtualDirectoryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/templates/".to_owned(),
                        },
                        config: None,
                        param: None,
                        params: None,
                        registry: RegistryArgs {
                            registry: VirtualDirectoryPath::LocalFolder {
                                path: "data/symbolic_test/".to_owned(),
                            },
                            follow_symlinks,
                            include_unreferenced: false,
                            v2: false,
                        },
                        policy: PolicyArgs {
                            policies: vec![],
                            skip_policies: true,
                            display_policy_coverage: false,
                        },
                        future: false,
                        diagnostic: Default::default(),
                    }),
                })),
            };

            let exit_directive = run_command(&cli);
            // The command should succeed in both cases
            assert_eq!(exit_directive.exit_code, 0);

            // Get the actual generated files
            let rust_files: std::collections::HashSet<_> = walkdir::WalkDir::new(&temp_output)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
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
                "File sets don't match for follow_symlinks = {follow_symlinks}"
            );
        }
    }

    #[test]
    fn test_registry_generate_v2() {
        let temp_output = Path::new("tests/v2_forge/observed_output");

        // Delete all the files in the observed_output/target directory
        // before generating the new files.
        std::fs::remove_dir_all(temp_output).unwrap_or_default();

        let cli = Cli {
            debug: 0,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Generate(RegistryGenerateArgs {
                    target: "markdown".to_owned(),
                    output: temp_output.to_path_buf(),
                    templates: VirtualDirectoryPath::LocalFolder {
                        path: "tests/v2_forge/templates/".to_owned(),
                    },
                    config: None,
                    param: None,
                    params: None,
                    registry: RegistryArgs {
                        registry: VirtualDirectoryPath::LocalFolder {
                            path: "tests/v2_forge/model/".to_owned(),
                        },
                        follow_symlinks: false,
                        include_unreferenced: false,
                        v2: true,
                    },
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: true,
                        display_policy_coverage: false,
                    },
                    future: false,
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli);
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);

        // validate expected = observed.
        let expected_output = Path::new("tests/v2_forge/expected_output");
        assert!(diff_dir(expected_output, temp_output).unwrap());
    }
}
