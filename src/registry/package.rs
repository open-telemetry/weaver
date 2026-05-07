// SPDX-License-Identifier: Apache-2.0

//! Package a semantic convention registry.

use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use clap::Args;
use log::info;
use weaver_common::log_success;

use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_semconv::manifest::{PublicationRegistryManifest, RegistryManifest};
use weaver_semconv::registry_repo::RegistryRepo;

use crate::registry::{load_config, Error, PolicyArgs, RegistryArgs};
use crate::weaver::WeaverEngine;
use crate::{DiagnosticArgs, ExitDirectives};
use weaver_common::http_auth::HttpAuthResolver;
use weaver_config::{WeaverCommand, WeaverConfig};
use weaver_macros::weaver_command;

/// Package a resolved registry for publication (produces `resolved.yaml` and `manifest.yaml`).
#[weaver_command(section = "package")]
#[derive(Debug, Args, WeaverCommand)]
pub struct RegistryPackageArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    #[shared(registry)]
    registry: RegistryArgs,

    /// Path to the directory where the package will be written.
    #[arg(short, long)]
    #[config(default = "output")]
    output: Option<PathBuf>,

    /// URI where the resolved schema will eventually be published.
    /// This value is embedded in the publication manifest as `resolved_schema_uri`.
    #[arg(long)]
    #[config]
    resolved_schema_uri: Option<String>,

    /// Policy parameters
    #[command(flatten)]
    #[shared(policy)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    #[shared(diagnostic)]
    pub diagnostic: DiagnosticArgs,
}

fn write_yaml(path: &Path, data: &impl serde::Serialize) -> Result<(), DiagnosticMessages> {
    let file = fs::File::create(path).map_err(|e| Error::OutputWrite {
        path: path.to_path_buf(),
        error: e.to_string(),
    })?;
    serde_yaml::to_writer(BufWriter::new(file), data).map_err(|e| Error::OutputWrite {
        path: path.to_path_buf(),
        error: e.to_string(),
    })?;
    Ok(())
}

/// Package a semantic convention registry.
pub(crate) fn command(
    args: &RegistryPackageArgs,
    cfg: Option<&WeaverConfig>,
    auth: &HttpAuthResolver,
) -> Result<ExitDirectives, DiagnosticMessages> {
    let cmd_config = load_config(args, cfg);
    let output = cmd_config.config.output;
    let resolved_schema_uri = cmd_config.config.resolved_schema_uri.ok_or_else(|| {
        DiagnosticMessages::from(Error::Config {
            error: "resolved_schema_uri is required (set via --resolved-schema-uri or [package] config)".to_owned(),
        })
    })?;
    info!("Packaging registry `{}`", cmd_config.registry.registry);

    // we only support packaging v2 registries
    if !cmd_config.registry.v2 {
        return Err(Error::PackagingRequiresV2.into());
    }

    let mut diag_msgs = DiagnosticMessages::empty();
    let weaver = WeaverEngine::new(&cmd_config.registry, &cmd_config.policy, auth);
    let registry_path = &cmd_config.registry.registry;

    let mut nfes = vec![];
    let repo = RegistryRepo::try_new_with_auth(None, registry_path, &mut nfes, auth)?;
    diag_msgs.extend_from_vec(nfes.into_iter().map(DiagnosticMessage::new).collect());

    // we require a definition manifest file to be present for packaging
    let manifest = repo
        .manifest()
        .ok_or_else(|| Error::PackagingRequiresManifest {
            registry: registry_path.to_string(),
        })?
        .clone();

    let definition_manifest = match manifest {
        RegistryManifest::Definition(m) => m,
        RegistryManifest::Publication(m) => {
            return Err(weaver_semconv::Error::UnexpectedPublicationManifest {
                schema_url: m.schema_url.to_string(),
            }
            .into());
        }
    };

    let loaded = weaver.load_definitions(repo, &mut diag_msgs)?;
    let resolved = weaver.resolve(loaded, &mut diag_msgs)?;

    let resolved_v2 = match resolved {
        crate::weaver::Resolved::V2(v) => v,
        crate::weaver::Resolved::V1(v) => v.try_into()?,
    };
    resolved_v2.check_after_resolution_policy(&mut diag_msgs)?;

    if diag_msgs.has_error() {
        return Err(diag_msgs);
    }

    fs::create_dir_all(&output).map_err(|e| Error::OutputWrite {
        path: output.clone(),
        error: e.to_string(),
    })?;
    let publication_manifest = PublicationRegistryManifest::try_from_registry_manifest(
        &definition_manifest,
        resolved_schema_uri,
    );

    write_yaml(&output.join("resolved.yaml"), resolved_v2.resolved_schema())?;
    write_yaml(&output.join("manifest.yaml"), &publication_manifest)?;

    log_success(format!(
        "Registry packaged successfully to `{}`",
        output.display()
    ));

    Ok(ExitDirectives {
        exit_code: 0,
        warnings: (!diag_msgs.is_empty()).then_some(diag_msgs),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_common::vdir::VirtualDirectoryPath;
    use weaver_semconv::manifest::PUBLICATION_MANIFEST_FILE_FORMAT;

    use crate::registry::{PolicyArgs, RegistryArgs};

    #[test]
    fn test_config_cli_consistency() {
        use crate::registry::tests::assert_config_cli_consistency;
        assert_config_cli_consistency::<RegistryPackageArgs>();
    }

    fn make_args(
        registry_path: &str,
        output: PathBuf,
        v2: bool,
        resolved_schema_uri: &str,
    ) -> RegistryPackageArgs {
        RegistryPackageArgs {
            registry: RegistryArgs {
                registry: Some(VirtualDirectoryPath::LocalFolder {
                    path: registry_path.to_owned(),
                }),
                v2: v2.then_some(true),
                ..Default::default()
            },
            output: Some(output),
            resolved_schema_uri: Some(resolved_schema_uri.to_owned()),
            policy: PolicyArgs {
                skip_policies: Some(true),
                ..Default::default()
            },
            diagnostic: Default::default(),
        }
    }

    /// A valid v2 registry with a manifest and a simple definition packages successfully.
    /// Both `resolved.yaml` and `manifest.yaml` are written to the output directory.
    #[test]
    fn test_package_happy_path() {
        let output = tempfile::tempdir().expect("failed to create tempdir");
        let args = make_args(
            "tests/package/valid_registry/",
            output.path().to_path_buf(),
            true,
            "https://test/semconv/1.0.0/resolved.yaml",
        );

        let result = command(&args, None, &HttpAuthResolver::empty());
        assert!(result.is_ok(), "Expected success, got: {result:?}");

        // resolved.yaml must exist, be valid YAML, and contain the v2 resolved schema format
        let resolved_path = output.path().join("resolved.yaml");
        assert!(resolved_path.exists(), "resolved.yaml not written");
        let resolved_content =
            fs::read_to_string(&resolved_path).expect("failed to read resolved.yaml");
        let resolved: serde_yaml::Value =
            serde_yaml::from_str(&resolved_content).expect("resolved.yaml is not valid YAML");
        assert_eq!(
            resolved["file_format"].as_str(),
            Some("resolved/2.0"),
            "resolved.yaml does not contain the expected v2 resolved schema file_format"
        );

        // manifest.yaml must exist and contain the correct fields
        let manifest_path = output.path().join("manifest.yaml");
        assert!(manifest_path.exists(), "manifest.yaml not written");
        let manifest = match RegistryManifest::try_from_file(&manifest_path, &mut vec![])
            .expect("manifest.yaml failed to load")
        {
            RegistryManifest::Publication(m) => m,
            other @ RegistryManifest::Definition(_) => {
                panic!("expected publication manifest, got {other:?}")
            }
        };

        assert_eq!(manifest.file_format, PUBLICATION_MANIFEST_FILE_FORMAT);
        assert_eq!(manifest.schema_url.as_str(), "https://test/schemas/1.0.0");
        assert_eq!(
            manifest.resolved_schema_uri,
            "https://test/semconv/1.0.0/resolved.yaml"
        );
        assert_eq!(
            manifest.description.as_deref(),
            Some("A test registry for packaging tests.")
        );
    }

    /// The output directory is created automatically if it does not exist.
    #[test]
    fn test_package_creates_output_dir() {
        let base = tempfile::tempdir().expect("failed to create tempdir");
        let nested_output = base.path().join("deep").join("nested").join("output");
        assert!(!nested_output.exists());

        let args = make_args(
            "tests/package/valid_registry/",
            nested_output.clone(),
            true,
            "https://test/semconv/1.0.0/resolved.yaml",
        );

        let result = command(&args, None, &HttpAuthResolver::empty());
        assert!(result.is_ok(), "Expected success, got: {result:?}");
        assert!(nested_output.exists(), "output directory was not created");
        assert!(nested_output.join("resolved.yaml").exists());
        assert!(nested_output.join("manifest.yaml").exists());
    }

    /// Packaging without the `--v2` flag fails immediately with `PackagingRequiresV2`.
    #[test]
    fn test_package_requires_v2_flag() {
        let output = tempfile::tempdir().expect("failed to create tempdir");
        let args = make_args(
            "tests/package/valid_registry/",
            output.path().to_path_buf(),
            false, // v2 = false
            "https://test/semconv/1.0.0/resolved.yaml",
        );

        let result = command(&args, None, &HttpAuthResolver::empty());
        assert!(result.is_err());
        let diag_msgs = result.unwrap_err();
        let msg = format!("{diag_msgs:?}");
        assert!(
            msg.contains("PackagingRequiresV2") || msg.contains("--v2"),
            "unexpected error: {msg}"
        );
    }

    /// Packaging a registry that has no manifest fails with `PackagingRequiresManifest`.
    #[test]
    fn test_package_requires_manifest() {
        let output = tempfile::tempdir().expect("failed to create tempdir");
        let args = make_args(
            "tests/package/no_manifest_registry/",
            output.path().to_path_buf(),
            true,
            "https://test/semconv/1.0.0/resolved.yaml",
        );

        let result = command(&args, None, &HttpAuthResolver::empty());
        assert!(result.is_err());
        let diag_msgs = result.unwrap_err();
        let msg = format!("{diag_msgs:?}");
        assert!(
            msg.contains("PackagingRequiresManifest") || msg.contains("manifest"),
            "unexpected error: {msg}"
        );
    }

    /// A definition file that references a nonexistent attribute fails during resolution.
    #[test]
    fn test_package_invalid_definition() {
        let output = tempfile::tempdir().expect("failed to create tempdir");
        let args = make_args(
            "tests/package/invalid_definition_registry/",
            output.path().to_path_buf(),
            true,
            "https://test/semconv/1.0.0/resolved.yaml",
        );

        let result = command(&args, None, &HttpAuthResolver::empty());
        assert!(
            result.is_err(),
            "Expected resolution failure for invalid definition"
        );
        // No output files should have been written
        assert!(!output.path().join("resolved.yaml").exists());
        assert!(!output.path().join("manifest.yaml").exists());
    }
}
