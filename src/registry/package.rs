// SPDX-License-Identifier: Apache-2.0

//! Package a semantic convention registry.

use std::fs;
use std::path::PathBuf;

use clap::Args;
use log::info;
use weaver_common::log_success;

use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_semconv::publication_manifest::PublicationRegistryManifest;
use weaver_semconv::registry_repo::RegistryRepo;

use crate::registry::{Error, PolicyArgs, RegistryArgs};
use crate::weaver::{ResolvedV2, WeaverEngine};
use crate::{DiagnosticArgs, ExitDirectives};

/// Parameters for the `registry package` sub-command
#[derive(Debug, Args)]
pub struct RegistryPackageArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Path to the directory where the package will be written.
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// URI where the resolved schema will eventually be published.
    /// This value is embedded in the publication manifest as `resolved_schema_uri`.
    #[arg(long)]
    resolved_schema_uri: String,

    /// Policy parameters
    #[command(flatten)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

/// Package a semantic convention registry.
pub(crate) fn command(args: &RegistryPackageArgs) -> Result<ExitDirectives, DiagnosticMessages> {
    info!("Packaging registry `{}`", args.registry.registry);

    // we only support packaging v2 registries
    if !args.registry.v2 {
        return Err(Error::PackagingRequiresV2.into());
    }

    let mut diag_msgs = DiagnosticMessages::empty();
    let weaver = WeaverEngine::new(&args.registry, &args.policy);
    let registry_path = &args.registry.registry;

    let mut nfes = vec![];
    let main_registry_repo = RegistryRepo::try_new(None, registry_path, &mut nfes)?;
    diag_msgs.extend_from_vec(nfes.into_iter().map(DiagnosticMessage::new).collect());

    // we require a manifest file to be present for packaging
    let registry_manifest = main_registry_repo
        .manifest()
        .ok_or_else(|| Error::PackagingRequiresManifest {
            registry: registry_path.to_string(),
        })?
        .clone();

    let loaded = weaver.load_definitions(main_registry_repo, &mut diag_msgs)?;
    let resolved = weaver.resolve(loaded, &mut diag_msgs)?;

    let resolved_v2: ResolvedV2 = resolved.try_into()?;
    resolved_v2.check_after_resolution_policy(&mut diag_msgs)?;

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    fs::create_dir_all(&args.output).map_err(|e| Error::InvalidParams {
        params_file: args.output.clone(),
        error: e.to_string(),
    })?;

    // Write resolved schema as resolved.yaml
    let resolved_path = args.output.join("resolved.yaml");
    let resolved_yaml = serde_yaml::to_string(&resolved_v2.resolved_schema())
        .map_err(|e| Error::InvalidParams {
            params_file: resolved_path.clone(),
            error: e.to_string(),
        })?;
    fs::write(&resolved_path, resolved_yaml).map_err(|e| Error::InvalidParams {
        params_file: resolved_path.clone(),
        error: e.to_string(),
    })?;

    // Build and write the publication manifest as manifest.yaml
    let publication_manifest = PublicationRegistryManifest::from_registry_manifest(
        &registry_manifest,
        args.resolved_schema_uri.clone(),
    );
    let manifest_path = args.output.join("manifest.yaml");
    let manifest_yaml =
        serde_yaml::to_string(&publication_manifest).map_err(|e| Error::InvalidParams {
            params_file: manifest_path.clone(),
            error: e.to_string(),
        })?;
    fs::write(&manifest_path, manifest_yaml).map_err(|e| Error::InvalidParams {
        params_file: manifest_path.clone(),
        error: e.to_string(),
    })?;

    log_success(format!(
        "Registry packaged successfully to `{}`",
        args.output.display()
    ));
    Ok(ExitDirectives {
        exit_code: 0,
        warnings: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_common::vdir::VirtualDirectoryPath;
    use weaver_semconv::publication_manifest::PUBLICATION_MANIFEST_FILE_FORMAT;

    use crate::registry::{PolicyArgs, RegistryArgs};

    fn make_args(
        registry_path: &str,
        output: PathBuf,
        v2: bool,
        resolved_schema_uri: &str,
    ) -> RegistryPackageArgs {
        RegistryPackageArgs {
            registry: RegistryArgs {
                registry: VirtualDirectoryPath::LocalFolder {
                    path: registry_path.to_owned(),
                },
                follow_symlinks: false,
                include_unreferenced: false,
                v2,
            },
            output,
            resolved_schema_uri: resolved_schema_uri.to_owned(),
            policy: PolicyArgs {
                policies: vec![],
                skip_policies: true,
                display_policy_coverage: false,
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

        let result = command(&args);
        assert!(result.is_ok(), "Expected success, got: {result:?}");

        // resolved.yaml must exist, be valid YAML, and contain the v2 resolved schema format
        let resolved_path = output.path().join("resolved.yaml");
        assert!(resolved_path.exists(), "resolved.yaml not written");
        let resolved_content = fs::read_to_string(&resolved_path).expect("failed to read resolved.yaml");
        let resolved: serde_yaml::Value = serde_yaml::from_str(&resolved_content)
            .expect("resolved.yaml is not valid YAML");
        assert_eq!(
            resolved["file_format"].as_str(),
            Some("resolved/2.0.0"),
            "resolved.yaml does not contain the expected v2 resolved schema file_format"
        );

        // manifest.yaml must exist and contain the correct fields
        let manifest_path = output.path().join("manifest.yaml");
        assert!(manifest_path.exists(), "manifest.yaml not written");
        let manifest_content = fs::read_to_string(&manifest_path).expect("failed to read manifest.yaml");
        let manifest: PublicationRegistryManifest =
            serde_yaml::from_str(&manifest_content).expect("manifest.yaml is not valid YAML");

        assert_eq!(manifest.file_format, PUBLICATION_MANIFEST_FILE_FORMAT);
        assert_eq!(manifest.schema_url.as_str(), "https://test/schemas/1.0.0");
        assert_eq!(manifest.resolved_schema_uri, "https://test/semconv/1.0.0/resolved.yaml");
        assert_eq!(manifest.description.as_deref(), Some("A test registry for packaging tests."));
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

        let result = command(&args);
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

        let result = command(&args);
        assert!(result.is_err());
        let diag_msgs = result.unwrap_err();
        let msg = format!("{diag_msgs:?}");
        assert!(msg.contains("PackagingRequiresV2") || msg.contains("--v2"), "unexpected error: {msg}");
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

        let result = command(&args);
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

        let result = command(&args);
        assert!(result.is_err(), "Expected resolution failure for invalid definition");
        // No output files should have been written
        assert!(!output.path().join("resolved.yaml").exists());
        assert!(!output.path().join("manifest.yaml").exists());
    }
}
