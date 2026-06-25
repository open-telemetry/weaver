// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::broken_intra_doc_links)]
#![doc = include_str!("../README.md")]

use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::fs::metadata;
use std::path::{Path, PathBuf};

use globset::Glob;
use miette::Diagnostic;
use serde::Serialize;
use serde_json::to_value;
use walkdir::DirEntry;

use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, handle_errors, WeaverError};

use crate::Error::CompoundError;

mod finding;

// Import finding so we don't need to expose deeper into the crate.
pub use crate::finding::FindingLevel;
pub use crate::finding::PolicyFinding;

/// Default semconv rules/functions for the semantic convention registry.
pub const SEMCONV_REGO: &str = include_str!("../../../defaults/rego/semconv.rego");

/// An error that can occur while evaluating policies.
#[derive(thiserror::Error, Debug, Serialize, Diagnostic, Clone)]
#[must_use]
#[non_exhaustive]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Error {
    /// An invalid policy.
    #[error("Invalid policy file '{file}', error: {error})")]
    #[diagnostic(
        url("https://www.openpolicyagent.org/docs/latest/policy-language/"),
        help("Check the policy file for syntax errors.")
    )]
    InvalidPolicyFile {
        /// The file that caused the error.
        file: String,
        /// The error that occurred.
        error: String,
    },

    /// An unsupported policy path.
    #[error("Invalid policy path '{path}'")]
    #[diagnostic(help("The specified path is neither a file nor a directory.”"))]
    UnsupportedPolicyPath {
        /// The path that caused the error.
        path: String,
    },

    /// Unable to access policy path.
    #[error("Invalid policy path '{path}'")]
    #[diagnostic(help(
        "Verify that the specified path exists and has the appropriate permissions."
    ))]
    AccessDenied {
        /// The path that caused the error.
        path: String,
        /// The error that occurred.
        error: String,
    },

    /// An invalid policy glob pattern.
    #[error("Invalid policy glob pattern '{pattern}', error: {error})")]
    #[diagnostic(
        url("https://docs.rs/globset/latest/globset/"),
        help("Check the glob pattern for syntax errors.")
    )]
    InvalidPolicyGlobPattern {
        /// The glob pattern that caused the error.
        pattern: String,
        /// The error that occurred.
        error: String,
    },

    /// An invalid data glob pattern.
    #[error("Invalid data glob pattern '{pattern}', error: {error})")]
    #[diagnostic(
        url("https://docs.rs/globset/latest/globset/"),
        help("Check the glob pattern for syntax errors.")
    )]
    InvalidDataGlobPattern {
        /// The glob pattern that caused the error.
        pattern: String,
        /// The error that occurred.
        error: String,
    },

    /// An invalid data.
    #[error("Invalid data, error: {error})")]
    #[diagnostic()]
    InvalidData {
        /// The error that occurred.
        error: String,
    },

    /// An invalid input.
    #[error("Invalid input, error: {error})")]
    #[diagnostic()]
    InvalidInput {
        /// The error that occurred.
        error: String,
    },

    /// Violation evaluation error.
    #[error("Violation evaluation error: {error}")]
    ViolationEvaluationError {
        /// The error that occurred.
        error: String,
    },

    /// A policy violation error.
    #[error("Policy violation: {violation}, provenance: {provenance}")]
    PolicyViolation {
        /// The provenance of the violation (URL or path).
        provenance: String,
        /// The violation.
        violation: Box<PolicyFinding>,
    },

    /// A container for multiple errors.
    #[error("{}", format_errors(.0))]
    #[diagnostic()]
    CompoundError(Vec<Error>),
}

impl WeaverError<Error> for Error {
    fn compound(errors: Vec<Error>) -> Error {
        Self::CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    Self::CompoundError(errors) => errors,
                    e => vec![e],
                })
                .collect(),
        )
    }
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(match error {
            CompoundError(errors) => errors
                .into_iter()
                .flat_map(|e| {
                    let diag_msgs: DiagnosticMessages = e.into();
                    diag_msgs.into_inner()
                })
                .collect(),
            _ => vec![DiagnosticMessage::new(error)],
        })
    }
}

/// A list of supported policy stages.
pub enum PolicyStage {
    /// Policies that are evaluated before resolution.
    BeforeResolution,
    /// Policies that are evaluated after resolution.
    AfterResolution,
    /// Policies that are evaluated between two registries the resolution phase.
    ComparisonAfterResolution,
    /// Policies that are evaluated to provide advice on samples.
    LiveCheckAdvice,
}

impl Display for PolicyStage {
    /// Returns the name of the policy stage.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyStage::BeforeResolution => {
                write!(f, "before_resolution")
            }
            PolicyStage::AfterResolution => {
                write!(f, "after_resolution")
            }
            PolicyStage::ComparisonAfterResolution => {
                write!(f, "comparison_after_resolution")
            }
            PolicyStage::LiveCheckAdvice => {
                write!(f, "live_check_advice")
            }
        }
    }
}

/// The policy engine.
#[derive(Clone, Default)]
pub struct Engine {
    // The `regorus` policy engine.
    engine: regorus::Engine,
    // Flag to enable the coverage report.
    coverage_enabled: bool,
    // Number of policy packages added.
    policy_package_count: usize,
    // Policy packages loaded. This is used to check if a policy package has been imported
    // before evaluating it.
    policy_packages: HashSet<String>,
}

impl Engine {
    /// Creates a new policy engine.
    #[must_use]
    pub fn new() -> Self {
        Default::default()
    }

    /// Enables the coverage report.
    pub fn enable_coverage(&mut self) {
        self.engine.set_enable_coverage(true);
        self.coverage_enabled = true;
    }

    /// Adds a rego policy (content) to the policy engine.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the policy (used for error messages).
    /// * `rego` - The content of the rego policy.
    ///
    /// # Returns
    ///
    /// The policy package name.
    pub fn add_policy(&mut self, path: &str, rego: &str) -> Result<String, Error> {
        let policy_package = self
            .engine
            .add_policy(path.to_owned(), rego.to_owned())
            .map_err(|e| Error::InvalidPolicyFile {
                file: path.to_owned(),
                error: e.to_string(),
            })
            .inspect(|_| {
                self.policy_package_count += 1;
            })?;
        // Add the policy package defined in the imported policy file.
        // Nothing prevent multiple policy files to import the same policy package.
        // All the rules will be combined and evaluated together.
        _ = self.policy_packages.insert(policy_package.clone());
        Ok(policy_package)
    }

    /// Adds a policy files to the policy engine. If path is a directory it will add any file matching *.rego
    /// A policy file is a `rego` file that contains the policies to be evaluated.
    ///
    /// # Arguments
    ///
    /// * `policy_path` - The path to the policy file or directory.
    ///
    /// # Returns
    ///
    /// The policy package name.
    pub fn add_policy_from_file_or_dir<P: AsRef<Path>>(
        &mut self,
        policy_path: P,
    ) -> Result<(), Error> {
        let path = policy_path.as_ref();

        let md = metadata(path).map_err(|err| Error::AccessDenied {
            path: path.to_string_lossy().to_string(),
            error: err.to_string(),
        })?;
        match (md.is_file(), md.is_dir()) {
            (true, _) => {
                _ = self.add_policy_from_file(path)?;
            }
            (false, true) => {
                _ = self.add_policies(path, "*.rego")?;
            }
            _ => {
                return Err(Error::UnsupportedPolicyPath {
                    path: path.to_string_lossy().to_string(),
                });
            }
        };
        Ok(())
    }

    /// Adds OPA data document from a JSON/YAML file, directory or a glob pattern.
    pub fn add_data_from_file_or_dir(&mut self, pattern: &str) -> Result<(), Error> {
        let (mut base, mut glob_pattern) = split_glob(pattern);

        // If the base path is empty, use the current directory.
        if base.as_os_str().is_empty() {
            base = PathBuf::from(".");
        }

        // Fail fast if the base path doesn't exist / isn't accessible.
        // Otherwise, WalkDir errors would be silently ignored or dropped by WalkDir.
        let md = metadata(&base).map_err(|err| Error::AccessDenied {
            path: base.to_string_lossy().to_string(),
            error: err.to_string(),
        })?;

        // If glob pattern is empty, load all files under base path.
        if glob_pattern.is_empty() {
            if md.is_file() {
                let root = base.parent().unwrap_or_else(|| Path::new("."));
                return self.add_data_file_from_dir(root, &base);
            }
            glob_pattern = "**/*".to_owned();
        }

        let glob = Glob::new(&glob_pattern)
            .map_err(|e| Error::InvalidDataGlobPattern {
                pattern: glob_pattern.clone(),
                error: e.to_string(),
            })?
            .compile_matcher();

        let mut errors = Vec::new();
        for entry in walkdir::WalkDir::new(&base)
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
        {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    errors.push(Error::AccessDenied {
                        path: base.to_string_lossy().to_string(),
                        error: err.to_string(),
                    });
                    continue;
                }
            };
            let entry_path = entry.path();
            if entry_path.is_file() {
                let rel_path = entry_path
                    .strip_prefix(&base)
                    .map_err(|e| Error::AccessDenied {
                        path: entry_path.to_string_lossy().to_string(),
                        error: format!("Failed to get relative path: {e}"),
                    })?;
                if glob.is_match(rel_path) {
                    if let Err(err) = self.add_data_file_from_dir(&base, entry_path) {
                        errors.push(err);
                    }
                }
            }
        }
        handle_errors(errors)?;
        Ok(())
    }

    fn add_data_file_from_dir<P1: AsRef<Path>, P2: AsRef<Path>>(
        &mut self,
        root_dir: P1,
        path: P2,
    ) -> Result<(), Error> {
        let root = root_dir.as_ref();
        let file_path = path.as_ref();

        let rel_path = file_path
            .strip_prefix(root)
            .map_err(|e| Error::InvalidData {
                error: format!("Failed to get relative path: {e}"),
            })?;

        let extension = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let content = std::fs::read_to_string(file_path).map_err(|e| Error::AccessDenied {
            path: file_path.to_string_lossy().to_string(),
            error: e.to_string(),
        })?;

        // OPA Rego requires data documents to be structured values. We deserialize the
        // content here to validate the structure and pass it to the engine.
        let parsed_value: serde_json::Value = match extension {
            "json" => {
                log::debug!("Including JSON data file {}", file_path.display());
                serde_json::from_str(&content).map_err(|e| Error::InvalidData {
                    error: format!("Invalid JSON file {}: {}", file_path.display(), e),
                })?
            }
            "yaml" | "yml" => {
                log::debug!("Including YAML data file {}", file_path.display());
                serde_yaml::from_str(&content).map_err(|e| Error::InvalidData {
                    error: format!("Invalid YAML file {}: {}", file_path.display(), e),
                })?
            }
            _ => {
                log::debug!(
                    "Skipping file {} (unsupported extension)",
                    file_path.display()
                );
                return Ok(());
            }
        };

        let mut components: Vec<String> = Vec::new();
        if let Some(parent) = rel_path.parent() {
            for comp in parent.components() {
                if let std::path::Component::Normal(os_str) = comp {
                    components.push(os_str.to_string_lossy().into_owned());
                }
            }
        }
        if let Some(stem) = file_path.file_stem() {
            components.push(stem.to_string_lossy().into_owned());
        }

        let mut wrapped = parsed_value;
        for key in components.into_iter().rev() {
            let mut map = serde_json::Map::new();
            let _ = map.insert(key, wrapped);
            wrapped = serde_json::Value::Object(map);
        }

        self.add_data(&wrapped)?;
        Ok(())
    }
    /// Adds a policy file to the policy engine.
    /// A policy file is a `rego` file that contains the policies to be evaluated.
    ///
    /// # Arguments
    ///
    /// * `policy_path` - The path to the policy file.
    ///
    /// # Returns
    ///
    /// The policy package name.
    pub fn add_policy_from_file<P: AsRef<Path>>(
        &mut self,
        policy_path: P,
    ) -> Result<String, Error> {
        let policy_path_str = policy_path.as_ref().to_string_lossy().to_string();

        let policy_package = self
            .engine
            .add_policy_from_file(policy_path)
            .map_err(|e| Error::InvalidPolicyFile {
                file: policy_path_str.clone(),
                error: e.to_string(),
            })
            .inspect(|_| {
                self.policy_package_count += 1;
            })?;
        // Add the policy package defined in the imported policy file.
        // Nothing prevent multiple policy files to import the same policy package.
        // All the rules will be combined and evaluated together.
        _ = self.policy_packages.insert(policy_package.clone());
        Ok(policy_package)
    }

    /// Adds all the policy files present in the given directory that match the
    /// given glob pattern (Unix-style glob syntax).
    ///
    /// Example of pattern: `*.rego`
    ///
    /// # Returns
    ///
    /// The number of policies added.
    pub fn add_policies<P: AsRef<Path>>(
        &mut self,
        policy_dir: P,
        policy_glob_pattern: &str,
    ) -> Result<usize, Error> {
        let mut errors = Vec::new();
        let mut added_policy_count = 0;

        let policy_glob = Glob::new(policy_glob_pattern)
            .map_err(|e| Error::InvalidPolicyGlobPattern {
                pattern: policy_glob_pattern.to_owned(),
                error: e.to_string(),
            })?
            .compile_matcher();

        let is_policy_file = |entry: &DirEntry| -> bool {
            let path = entry.path().to_string_lossy();
            policy_glob.is_match(path.as_ref())
        };

        // Visit recursively all the files in the policy directory
        for entry in walkdir::WalkDir::new(policy_dir)
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
            .flatten()
        {
            if is_policy_file(&entry) {
                if let Err(err) = self.add_policy_from_file(entry.path()) {
                    errors.push(err);
                } else {
                    added_policy_count += 1;
                }
            }
        }

        handle_errors(errors)?;

        Ok(added_policy_count)
    }

    /// Returns the number of policy packages added to the policy engine.
    #[must_use]
    pub fn policy_package_count(&self) -> usize {
        self.policy_package_count
    }

    /// Adds a data document to the policy engine.
    ///
    /// Data versus Input: In essence, data is about what the policy engine
    /// knows globally and statically (or what is updated dynamically but
    /// considered part of policy engine's world knowledge), while input is
    /// about what each request or query brings to the policy engine at
    /// runtime, needing a decision based on current, external circumstances.
    /// Combining data and input allows the policy engine to make informed,
    /// context-aware decisions based on both its internal knowledge base and
    /// the specifics of each request or action being evaluated.
    pub fn add_data<T: Serialize>(&mut self, data: &T) -> Result<(), Error> {
        let json_data = to_value(data).map_err(|e| Error::InvalidData {
            error: e.to_string(),
        })?;
        let value: regorus::Value =
            serde_json::from_value(json_data).map_err(|e| Error::InvalidInput {
                error: e.to_string(),
            })?;
        self.engine.add_data(value).map_err(|e| Error::InvalidData {
            error: e.to_string(),
        })
    }

    /// Clears the data from the policy engine.
    pub fn clear_data(&mut self) {
        self.engine.clear_data();
    }

    /// Sets an input document for the policy engine.
    ///
    /// Data versus Input: In essence, data is about what the policy engine
    /// knows globally and statically (or what is updated dynamically but
    /// considered part of policy engine's world knowledge), while input is
    /// about what each request or query brings to the policy engine at
    /// runtime, needing a decision based on current, external circumstances.
    /// Combining data and input allows the policy engine to make informed,
    /// context-aware decisions based on both its internal knowledge base and
    /// the specifics of each request or action being evaluated.
    pub fn set_input<T: Serialize>(&mut self, input: &T) -> Result<(), Error> {
        let json_input = to_value(input).map_err(|e| Error::InvalidInput {
            error: e.to_string(),
        })?;

        let value: regorus::Value =
            serde_json::from_value(json_input).map_err(|e| Error::InvalidInput {
                error: e.to_string(),
            })?;
        self.engine.set_input(value);
        Ok(())
    }

    /// Returns true if there are any policy packages for a given stage.
    #[must_use]
    pub fn has_stage(&self, stage: PolicyStage) -> bool {
        self.policy_packages.contains(&format!("data.{stage}"))
    }

    /// Returns a list of violations based on the policies, the data, the
    /// input, and the given policy stage.
    #[allow(clippy::print_stdout)] // Used to display the coverage (debugging purposes only)
    pub fn check(&mut self, stage: PolicyStage) -> Result<Vec<PolicyFinding>, Error> {
        // If we don't have any policy package that matches the stage,
        // return an empty list of violations.
        if !self.policy_packages.contains(&format!("data.{stage}")) {
            return Ok(vec![]);
        }

        let value = self
            .engine
            .eval_rule(format!("data.{stage}.deny"))
            .map_err(|e| Error::ViolationEvaluationError {
                error: e.to_string(),
            })?;

        // Print the coverage report if enabled
        // This is useful for debugging purposes
        if self.coverage_enabled {
            let report =
                self.engine
                    .get_coverage_report()
                    .map_err(|e| Error::ViolationEvaluationError {
                        error: e.to_string(),
                    })?;
            let pretty_report =
                report
                    .to_string_pretty()
                    .map_err(|e| Error::ViolationEvaluationError {
                        error: e.to_string(),
                    })?;
            println!("{pretty_report}");
        }

        // convert `regorus` value to `serde_json` value
        let json_value = to_value(&value).map_err(|e| Error::ViolationEvaluationError {
            error: e.to_string(),
        })?;

        // convert json value into a vector of violations
        let violations: Vec<PolicyFinding> =
            serde_json::from_value(json_value).map_err(|e| Error::ViolationEvaluationError {
                error: e.to_string(),
            })?;

        Ok(violations)
    }
}

fn split_glob(pattern: &str) -> (PathBuf, String) {
    let path = Path::new(pattern);
    let mut base = PathBuf::new();
    let mut rest = Vec::new();
    let mut found_wildcard = false;

    for component in path.components() {
        let component_str = component.as_os_str().to_string_lossy();
        if found_wildcard
            || component_str.contains('*')
            || component_str.contains('?')
            || component_str.contains('[')
        {
            found_wildcard = true;
            rest.push(component_str.into_owned());
        } else {
            base.push(component);
        }
    }

    if rest.is_empty() {
        (base, "".to_owned())
    } else {
        (base, rest.join("/"))
    }
}

fn is_hidden(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_yaml::Value;

    use weaver_common::error::format_errors;

    use crate::finding::PolicyFinding;
    use crate::{split_glob, Engine, Error, PolicyStage};

    #[test]
    fn test_policy() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = Engine::new();
        let policy_package = engine.add_policy_from_file("data/policies/otel_policies.rego")?;
        assert_eq!(policy_package, "data.before_resolution");

        let old_semconv = std::fs::read_to_string("data/registries/registry.network.old.yaml")?;
        let old_semconv: Value = serde_yaml::from_str(&old_semconv)?;
        engine.add_data(&old_semconv)?;

        let new_semconv = std::fs::read_to_string("data/registries/registry.network.new.yaml")?;
        let new_semconv: Value = serde_yaml::from_str(&new_semconv)?;
        engine.set_input(&new_semconv)?;

        let expected_violations: HashMap<String, PolicyFinding> = vec![
            PolicyFinding::new_semconv_attribute(
                "attr_stability_deprecated".to_owned(),
                "attribute".to_owned(),
                "registry.network1".to_owned(),
                "protocol.name".to_owned(),
            ),
            PolicyFinding {
                id: "attr_removed".to_owned(),
                context: Some(serde_json::json!({
                    "id": "schema_evolution",
                    "group": "registry.network1".to_owned(),
                    "attr": "protocol.name.3".to_owned(),
                })),
                message: "Schema evolution violation".to_owned(),
                level: crate::finding::FindingLevel::Violation,
                signal_type: None,
                signal_name: None,
            },
            PolicyFinding::new_semconv_attribute(
                "registry_with_ref_attr".to_owned(),
                "attribute_registry".to_owned(),
                "registry.network1".to_owned(),
                "protocol.port".to_owned(),
            ),
        ]
        .into_iter()
        .map(|v| (make_id_for_semconv_attribute(&v), v))
        .collect();

        let violations = engine.check(PolicyStage::BeforeResolution)?;
        assert_eq!(violations.len(), 3);

        for violation in violations {
            assert_eq!(
                expected_violations.get(&make_id_for_semconv_attribute(&violation)),
                Some(&violation)
            );
        }

        Ok(())
    }

    #[test]
    fn test_invalid_policy() {
        let mut engine = Engine::new();
        let result = engine.add_policy_from_file("data/policies/invalid_policy.rego");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_data() {
        let mut engine = Engine::new();
        let result = engine.add_data(&"invalid data");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_violation_object() {
        let mut engine = Engine::new();
        _ = engine
            .add_policy_from_file("data/policies/invalid_violation_object.rego")
            .unwrap();

        let new_semconv =
            std::fs::read_to_string("data/registries/registry.network.new.yaml").unwrap();
        let new_semconv: Value = serde_yaml::from_str(&new_semconv).unwrap();
        engine.set_input(&new_semconv).unwrap();

        let result = engine.check(PolicyStage::BeforeResolution);
        assert!(result.is_err());

        let observed_errors = format_errors(&[result.unwrap_err()]);
        assert_eq!(
            observed_errors,
            "Violation evaluation error: missing field `level`"
        );
    }

    fn make_id_for_semconv_attribute(v: &PolicyFinding) -> String {
        format!(
            "{}-{}",
            v.id,
            v.context
                .as_ref()
                .unwrap()
                .as_object()
                .unwrap()
                .get("id")
                .unwrap()
        )
    }
    #[test]
    fn test_add_policies() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = Engine::new();
        let result = engine.add_policies("data/registries", "*.rego");

        assert!(result.is_ok());

        let old_semconv = std::fs::read_to_string("data/registries/registry.network.old.yaml")?;
        let old_semconv: Value = serde_yaml::from_str(&old_semconv)?;
        engine.add_data(&old_semconv)?;

        let new_semconv = std::fs::read_to_string("data/registries/registry.network.new.yaml")?;
        let new_semconv: Value = serde_yaml::from_str(&new_semconv)?;
        engine.set_input(&new_semconv)?;

        let expected_violations: HashMap<String, PolicyFinding> = vec![
            PolicyFinding::new_semconv_attribute(
                "attr_stability_deprecated".to_owned(),
                "attribute".to_owned(),
                "registry.network1".to_owned(),
                "protocol.name".to_owned(),
            ),
            PolicyFinding::new_semconv_attribute(
                "attr_removed".to_owned(),
                "schema_evolution".to_owned(),
                "registry.network1".to_owned(),
                "protocol.name.3".to_owned(),
            ),
            PolicyFinding::new_semconv_attribute(
                "registry_with_ref_attr".to_owned(),
                "attribute_registry".to_owned(),
                "registry.network1".to_owned(),
                "protocol.port".to_owned(),
            ),
        ]
        .into_iter()
        .map(|v| (make_id_for_semconv_attribute(&v), v))
        .collect();

        let violations = engine.check(PolicyStage::BeforeResolution)?;
        assert_eq!(violations.len(), 3);

        for violation in violations {
            assert_eq!(
                expected_violations.get(&make_id_for_semconv_attribute(&violation)),
                Some(&violation)
            );
        }

        Ok(())
    }

    #[test]
    fn test_add_policies_with_invalid_policies() {
        let mut engine = Engine::new();
        let result = engine.add_policies("data/policies", "*.rego");

        // We have 2 invalid Rego files in data/policies
        assert!(result.is_err());
        if let Error::CompoundError(errors) = result.err().unwrap() {
            assert_eq!(errors.len(), 2, "Found errors: {errors:?}");
        } else {
            panic!("Expected a CompoundError");
        }
    }

    #[test]
    fn test_policy_from_file_or_dir() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = Engine::new();
        engine.add_policy_from_file_or_dir("data/policies/otel_policies.rego")?;
        assert_eq!(1, engine.policy_package_count);

        engine.add_policy_from_file_or_dir("data/multi-policies")?;
        assert_eq!(3, engine.policy_package_count);
        Ok(())
    }

    #[test]
    fn test_can_determine_before_resolution_policy() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = Engine::new();
        assert!(!engine.has_stage(PolicyStage::BeforeResolution));
        engine.add_policy_from_file_or_dir("data/multi-policies")?;
        assert!(engine.has_stage(PolicyStage::BeforeResolution));
        Ok(())
    }

    #[test]
    fn test_add_policy_from_file_or_dir_with_nested_data() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path();
        let policy_path = temp_path.join("policy.rego");
        let schemas_dir = temp_path.join("schemas");
        let _ = std::fs::create_dir_all(&schemas_dir);
        let schema_path = schemas_dir.join("user.json");

        let rego_content = r#"
            package before_resolution

            import rego.v1

            deny contains {
                "id": "user_id_invalid",
                "message": sprintf("User ID '%v' is invalid", [input.user_id]),
                "level": "violation"
            } if {
                input.user_id
                not is_number(input.user_id)
                data.schemas.user.properties.user_id.type == "number"
            }
        "#;
        std::fs::write(&policy_path, rego_content)?;

        let json_content = r#"
            {
                "properties": {
                    "user_id": {
                        "type": "number"
                    }
                }
            }
        "#;
        std::fs::write(&schema_path, json_content)?;

        let mut engine = Engine::new();
        engine.add_policy_from_file_or_dir(temp_path)?;
        let glob_pattern = format!("{}/**/*.json", temp_path.to_str().unwrap());
        engine.add_data_from_file_or_dir(&glob_pattern)?;

        // Set input that triggers the rule (user_id is a string, not a number)
        let input = serde_json::json!({
            "user_id": "not-a-number"
        });
        engine.set_input(&input)?;

        let violations = engine.check(PolicyStage::BeforeResolution)?;
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].id, "user_id_invalid");

        Ok(())
    }

    #[test]
    fn test_add_data_from_file_or_dir_with_paths() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path();

        let policy_path = temp_path.join("policy.rego");
        let schemas_dir = temp_path.join("schemas");
        let _ = std::fs::create_dir_all(&schemas_dir);
        let user_schema_path = schemas_dir.join("user.json");
        let admin_schema_path = schemas_dir.join("admin.yaml");

        let rego_content = r#"
            package before_resolution
            import rego.v1
            deny contains {
                "id": "test_err",
                "message": "test error",
                "level": "violation"
            } if {
                data.user.properties.user_id.type == "number"
                data.admin.properties.role.type == "string"
            }
        "#;
        std::fs::write(&policy_path, rego_content)?;

        let user_json = r#"{"properties": {"user_id": {"type": "number"}}}"#;
        std::fs::write(&user_schema_path, user_json)?;

        let admin_yaml = r#"
            properties:
              role:
                type: string
        "#;
        std::fs::write(&admin_schema_path, admin_yaml)?;

        // Test loading direct directory path (with no wildcards)
        let mut engine_dir = Engine::new();
        engine_dir.add_policy_from_file_or_dir(temp_path)?;
        engine_dir.add_data_from_file_or_dir(schemas_dir.to_str().unwrap())?;
        let violations = engine_dir.check(PolicyStage::BeforeResolution)?;
        assert!(!violations.is_empty());

        // Test loading direct individual file paths
        let mut engine_files = Engine::new();
        engine_files.add_policy_from_file_or_dir(temp_path)?;
        engine_files.add_data_from_file_or_dir(user_schema_path.to_str().unwrap())?;
        engine_files.add_data_from_file_or_dir(admin_schema_path.to_str().unwrap())?;
        let violations = engine_files.check(PolicyStage::BeforeResolution)?;
        assert!(!violations.is_empty());

        Ok(())
    }

    #[test]
    fn test_split_glob() {
        let cases = vec![
            ("schemas/*.json", "schemas", "*.json"),
            ("schemas/**/*.json", "schemas", "**/*.json"),
            ("user.json", "user.json", ""),
            (
                "/absolute/path/to/schemas/*.json",
                "/absolute/path/to/schemas",
                "*.json",
            ),
            ("a/b/c/d", "a/b/c/d", ""),
        ];

        for (pattern, expected_base, expected_glob) in cases {
            let (base, glob) = split_glob(pattern);
            assert_eq!(
                base.to_string_lossy().replace('\\', "/"),
                expected_base,
                "Pattern: {}",
                pattern
            );
            assert_eq!(glob, expected_glob, "Pattern: {}", pattern);
        }
    }

    #[test]
    fn test_add_data_from_file_or_dir_invalid_paths() {
        let mut engine = Engine::new();
        // A non-existent file/directory should return an AccessDenied error
        let result = engine.add_data_from_file_or_dir("/non/existent/path/to/data");
        assert!(matches!(result, Err(Error::AccessDenied { .. })));

        // A non-existent base path for a glob pattern should also return AccessDenied
        let result = engine.add_data_from_file_or_dir("/non/existent/path/*.json");
        assert!(matches!(result, Err(Error::AccessDenied { .. })));
    }
}
