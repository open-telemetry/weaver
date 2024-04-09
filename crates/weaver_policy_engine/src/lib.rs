// SPDX-License-Identifier: Apache-2.0

//! This crate integrates a general purpose policy engine with the Weaver
//! project. The project `regorus` is the policy engine used in this crate to
//! evaluate policies.

use crate::violation::Violation;
use serde::Serialize;
use serde_json::to_value;
use std::path::Path;

pub mod violation;

/// An error that can occur while evaluating policies.
#[derive(thiserror::Error, Debug)]
#[must_use]
#[non_exhaustive]
pub enum Error {
    /// An invalid policy.
    #[error("Invalid policy file '{file}', error: {error})")]
    InvalidPolicyFile {
        /// The file that caused the error.
        file: String,
        /// The error that occurred.
        error: String,
    },

    /// An invalid data.
    #[error("Invalid data, error: {error})")]
    InvalidData {
        /// The error that occurred.
        error: String,
    },

    /// An invalid input.
    #[error("Invalid input, error: {error})")]
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

    /// A container for multiple errors.
    #[error("{:?}", Error::format_errors(.0))]
    CompoundError(Vec<Error>),
}

impl Error {
    /// Formats the given errors into a single string.
    /// This used to render compound errors.
    #[must_use]
    pub fn format_errors(errors: &[Error]) -> String {
        errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<String>>()
            .join("\n\n")
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

    /// Returns a list of violations based on the policies, the data, and the
    /// input.
    pub fn check(&mut self) -> Result<Vec<Violation>, Error> {
        let result = self
            .engine
            .eval_query("data.otel.deny".to_owned(), false)
            .map_err(|e| Error::ViolationEvaluationError {
                error: e.to_string(),
            })?;

        let mut violations = Vec::new();

        for query_result in result.result {
            for expr in query_result.expressions {
                // convert `regorus` value to `serde_json` value
                let json_value =
                    to_value(&expr.value).map_err(|e| Error::ViolationEvaluationError {
                        error: e.to_string(),
                    })?;

                // convert json value into a vector of violations
                let violation: Vec<Violation> =
                    serde_json::from_value(json_value).map_err(|e| {
                        Error::ViolationEvaluationError {
                            error: e.to_string(),
                        }
                    })?;

                violations.extend(violation);
            }
        }
        Ok(violations)
    }
}

#[cfg(test)]
mod tests {
    use crate::violation::Violation;
    use crate::{Engine, Error};
    use serde_yaml::Value;
    use std::collections::HashMap;

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

        let violations = engine.check()?;
        assert_eq!(violations.len(), 3);

        for violation in violations {
            assert_eq!(expected_violations.get(violation.id()), Some(&violation));
            println!("{}", violation);
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
        engine.add_policy("data/policies/invalid_violation_object.rego").unwrap();

        let new_semconv = std::fs::read_to_string("data/registries/registry.network.new.yaml").unwrap();
        let new_semconv: Value = serde_yaml::from_str(&new_semconv).unwrap();
        engine.set_input(&new_semconv).unwrap();

        let result = engine.check();
        assert!(result.is_err());

        let observed_errors = Error::format_errors(&[result.unwrap_err()]);
        assert_eq!(
            observed_errors,
            "Violation evaluation error: missing field `type`"
        );
    }
}
