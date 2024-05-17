// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]

use std::fmt::{Display, Formatter};
use std::path::Path;

use globset::Glob;
use miette::Diagnostic;
use serde::Serialize;
use serde_json::to_value;
use walkdir::DirEntry;

use weaver_common::error::{format_errors, handle_errors, WeaverError};

use crate::violation::Violation;
use crate::Error::CompoundError;

pub mod violation;

/// An error that can occur while evaluating policies.
#[derive(thiserror::Error, Debug, Serialize, Diagnostic, Clone)]
#[must_use]
#[non_exhaustive]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Error {
    /// An invalid policy.
    #[error("Invalid policy file '{file}', error: {error})")]
    #[diagnostic(
        severity = "error",
        url("https://www.openpolicyagent.org/docs/latest/policy-language/"),
        help("Check the policy file for syntax errors.")
    )]
    InvalidPolicyFile {
        /// The file that caused the error.
        file: String,
        /// The error that occurred.
        error: String,
    },

    /// An invalid policy glob pattern.
    #[error("Invalid policy glob pattern '{pattern}', error: {error})")]
    #[diagnostic(
        severity = "error",
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
    #[diagnostic(severity = "error")]
    ViolationEvaluationError {
        /// The error that occurred.
        error: String,
    },

    /// A policy violation error.
    #[error("Policy violation: {violation}, provenance: {provenance}")]
    #[diagnostic(severity = "error")]
    PolicyViolation {
        /// The provenance of the violation (URL or path).
        provenance: String,
        /// The violation.
        violation: Violation,
    },

    /// A container for multiple errors.
    #[error("{:?}", format_errors(.0))]
    #[diagnostic()]
    CompoundError(Vec<Error>),
}

impl WeaverError<Error> for Error {
    /// Retrieves a list of error messages associated with this error.
    fn errors(&self) -> Vec<String> {
        match self {
            CompoundError(errors) => errors.iter().flat_map(WeaverError::errors).collect(),
            _ => vec![self.to_string()],
        }
    }
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

/// A list of supported policy packages.
pub enum PolicyPackage {
    /// Policies that are evaluated before resolution.
    BeforeResolution,
    /// Policies that are evaluated after resolution.
    AfterResolution,
}

impl Display for PolicyPackage {
    /// Returns the name of the policy package.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyPackage::BeforeResolution => {
                write!(f, "before_resolution")
            }
            PolicyPackage::AfterResolution => {
                write!(f, "after_resolution")
            }
        }
    }
}

/// The policy engine.
#[derive(Clone, Default)]
pub struct Engine {
    // The `regorus` policy engine.
    engine: regorus::Engine,
}

impl Engine {
    /// Creates a new policy engine.
    #[must_use]
    pub fn new() -> Self {
        Default::default()
    }

    /// Adds a policy file to the policy engine.
    /// A policy file is a `rego` file that contains the policies to be evaluated.
    ///
    /// # Arguments
    ///
    /// * `policy_path` - The path to the policy file.
    pub fn add_policy<P: AsRef<Path>>(&mut self, policy_path: P) -> Result<(), Error> {
        let policy_path_str = policy_path.as_ref().to_string_lossy().to_string();

        self.engine
            .add_policy_from_file(policy_path)
            .map_err(|e| Error::InvalidPolicyFile {
                file: policy_path_str.clone(),
                error: e.to_string(),
            })
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
                if let Err(err) = self.add_policy(entry.path()) {
                    errors.push(err);
                } else {
                    added_policy_count += 1;
                }
            }
        }

        handle_errors(errors)?;

        Ok(added_policy_count)
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

    /// Returns a list of violations based on the policies, the data, the
    /// input, and the given package.
    pub fn check(&mut self, package: PolicyPackage) -> Result<Vec<Violation>, Error> {
        let value = self
            .engine
            .eval_rule(format!("data.{}.deny", package))
            .map_err(|e| Error::ViolationEvaluationError {
                error: e.to_string(),
            })?;

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
    use crate::{Engine, Error, PolicyPackage};

    #[test]
    fn test_policy() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = Engine::new();
        engine.add_policy("data/policies/otel_policies.rego")?;

        let old_semconv = std::fs::read_to_string("data/registries/registry.network.old.yaml")?;
        let old_semconv: Value = serde_yaml::from_str(&old_semconv)?;
        engine.add_data(&old_semconv)?;

        let new_semconv = std::fs::read_to_string("data/registries/registry.network.new.yaml")?;
        let new_semconv: Value = serde_yaml::from_str(&new_semconv)?;
        engine.set_input(&new_semconv)?;

        let expected_violations: HashMap<String, Violation> = vec![
            Violation::SemconvAttribute {
                id: "attr_stability_deprecated".to_owned(),
                category: "attrigute".to_owned(),
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
                category: "attrigute_registry".to_owned(),
                group: "registry.network1".to_owned(),
                attr: "protocol.port".to_owned(),
            },
        ]
        .into_iter()
        .map(|v| (v.id().to_owned(), v))
        .collect();

        let violations = engine.check(PolicyPackage::BeforeResolution)?;
        assert_eq!(violations.len(), 3);

        for violation in violations {
            assert_eq!(expected_violations.get(violation.id()), Some(&violation));
        }

        Ok(())
    }

    #[test]
    fn test_invalid_policy() {
        let mut engine = Engine::new();
        let result = engine.add_policy("data/policies/invalid_policy.rego");
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
        engine
            .add_policy("data/policies/invalid_violation_object.rego")
            .unwrap();

        let new_semconv =
            std::fs::read_to_string("data/registries/registry.network.new.yaml").unwrap();
        let new_semconv: Value = serde_yaml::from_str(&new_semconv).unwrap();
        engine.set_input(&new_semconv).unwrap();

        let result = engine.check(PolicyPackage::BeforeResolution);
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
                category: "attrigute".to_owned(),
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
                category: "attrigute_registry".to_owned(),
                group: "registry.network1".to_owned(),
                attr: "protocol.port".to_owned(),
            },
        ]
        .into_iter()
        .map(|v| (v.id().to_owned(), v))
        .collect();

        let violations = engine.check(PolicyPackage::BeforeResolution)?;
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
}
