// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::broken_intra_doc_links)]
#![doc = include_str!("../README.md")]

use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::fs::metadata;
use std::path::Path;

use globset::Glob;
use miette::Diagnostic;
use serde::Serialize;
use serde_json::to_value;
use walkdir::DirEntry;

use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, handle_errors, WeaverError};

use crate::violation::Violation;
use crate::Error::CompoundError;

pub mod violation;

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
        violation: Box<Violation>,
    },

    /// A container for multiple errors.
    #[error("{:?}", format_errors(.0))]
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
        fn is_hidden(entry: &DirEntry) -> bool {
            entry
                .file_name()
                .to_str()
                .map(|s| s.starts_with('.'))
                .unwrap_or(false)
        }

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
        for entry in walkdir::WalkDir::new(policy_dir).into_iter().flatten() {
            if is_hidden(&entry) {
                continue;
            }
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
    pub fn check(&mut self, stage: PolicyStage) -> Result<Vec<Violation>, Error> {
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
        let violations: Vec<Violation> =
            serde_json::from_value(json_value).map_err(|e| Error::ViolationEvaluationError {
                error: e.to_string(),
            })?;

        Ok(violations)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_yaml::Value;

    use weaver_common::error::format_errors;

    use crate::violation::Violation;
    use crate::{Engine, Error, PolicyStage};

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

        let expected_violations: HashMap<String, Violation> = vec![
            Violation::SemconvAttribute {
                id: "attr_stability_deprecated".to_owned(),
                category: "attribute".to_owned(),
                group: "registry.network1".to_owned(),
                attr: "protocol.name".to_owned(),
            },
            Violation::SemconvAttribute {
                id: "attr_removed".to_owned(),
                category: "schema_evolution".to_owned(),
                group: "registry.network1".to_owned(),
                attr: "protocol.name.3".to_owned(),
            },
            Violation::SemconvAttribute {
                id: "registry_with_ref_attr".to_owned(),
                category: "attribute_registry".to_owned(),
                group: "registry.network1".to_owned(),
                attr: "protocol.port".to_owned(),
            },
        ]
        .into_iter()
        .map(|v| (v.id().to_owned(), v))
        .collect();

        let violations = engine.check(PolicyStage::BeforeResolution)?;
        assert_eq!(violations.len(), 3);

        for violation in violations {
            assert_eq!(expected_violations.get(violation.id()), Some(&violation));
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
            "Violation evaluation error: missing field `type`"
        );
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

        let expected_violations: HashMap<String, Violation> = vec![
            Violation::SemconvAttribute {
                id: "attr_stability_deprecated".to_owned(),
                category: "attribute".to_owned(),
                group: "registry.network1".to_owned(),
                attr: "protocol.name".to_owned(),
            },
            Violation::SemconvAttribute {
                id: "attr_removed".to_owned(),
                category: "schema_evolution".to_owned(),
                group: "registry.network1".to_owned(),
                attr: "protocol.name.3".to_owned(),
            },
            Violation::SemconvAttribute {
                id: "registry_with_ref_attr".to_owned(),
                category: "attribute_registry".to_owned(),
                group: "registry.network1".to_owned(),
                attr: "protocol.port".to_owned(),
            },
        ]
        .into_iter()
        .map(|v| (v.id().to_owned(), v))
        .collect();

        let violations = engine.check(PolicyStage::BeforeResolution)?;
        assert_eq!(violations.len(), 3);

        for violation in violations {
            assert_eq!(expected_violations.get(violation.id()), Some(&violation));
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
            assert_eq!(errors.len(), 2);
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
}
