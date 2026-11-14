// SPDX-License-Identifier: Apache-2.0

//! Rego policy-based advisor

use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::Rc,
};
use weaver_checker::{Engine, PolicyFinding};
use weaver_forge::jq;

use super::{emit_findings, Advisor};
use crate::{
    live_checker::LiveChecker, otlp_logger::OtlpEmitter,
    sample_instrumentation_scope::SampleInstrumentationScope, sample_resource::SampleResource,
    Error, Sample, SampleRef, VersionedAttribute, VersionedSignal, DEFAULT_LIVE_CHECK_JQ,
    DEFAULT_LIVE_CHECK_REGO, DEFAULT_LIVE_CHECK_REGO_POLICY_PATH,
};

/// The `--advice-policies` value that names the built-in policies.
pub const BUILTIN_ADVICE_POLICIES: &str = "builtin://otel.rego";

const BUILTIN_SCHEME: &str = "builtin://";

/// A source of rego advice policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdvicePolicySource {
    /// The policies built into weaver.
    Builtin,
    /// A `.rego` file or a directory of them.
    Path(PathBuf),
}

impl AdvicePolicySource {
    /// Returns the built-in source that `source` names, or `None` when `source` is not a `builtin://` URI.
    pub fn builtin(source: &str) -> Result<Option<Self>, Error> {
        if source == BUILTIN_ADVICE_POLICIES {
            Ok(Some(Self::Builtin))
        } else if source.starts_with(BUILTIN_SCHEME) {
            Err(Error::AdviceError {
                error: format!(
                    "Unknown built-in advice policies `{source}`; expected `{BUILTIN_ADVICE_POLICIES}`."
                ),
            })
        } else {
            Ok(None)
        }
    }
}

impl From<PathBuf> for AdvicePolicySource {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

impl From<&Path> for AdvicePolicySource {
    fn from(path: &Path) -> Self {
        Self::Path(path.to_path_buf())
    }
}

/// An advisor which runs a rego policy on the attribute
#[derive(Clone)]
pub struct RegoAdvisor {
    engine: Engine,
}

impl RegoAdvisor {
    /// Creates a RegoAdvisor from `policies`, or from the built-in policies when there are none.
    pub fn new(
        live_checker: &LiveChecker,
        policies: &[AdvicePolicySource],
        jq_preprocessor: &Option<PathBuf>,
        advice_data: &Option<String>,
    ) -> Result<Self, Error> {
        let mut engine = Engine::new();
        // The built-in policies load first: regorus panics when a later module redefines one of their functions.
        if policies.is_empty() || policies.contains(&AdvicePolicySource::Builtin) {
            let _ = engine
                .add_policy(DEFAULT_LIVE_CHECK_REGO_POLICY_PATH, DEFAULT_LIVE_CHECK_REGO)
                .map_err(|e| Error::AdviceError {
                    error: e.to_string(),
                })?;
        }
        for source in policies {
            if let AdvicePolicySource::Path(path) = source {
                engine
                    .add_policy_from_file_or_dir(path)
                    .map_err(|e| Error::AdviceError {
                        error: e.to_string(),
                    })?;
            }
        }

        if let Some(pattern) = advice_data {
            engine
                .add_data_from_file_or_dir(pattern)
                .map_err(|e| Error::AdviceError {
                    error: e.to_string(),
                })?;
        }

        // If there is a jq preprocessor then pass the live_checker data through it before adding it to the engine
        // Otherwise use the default jq preprocessor
        let jq_filter = if let Some(path) = jq_preprocessor {
            std::fs::read_to_string(path).map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?
        } else {
            DEFAULT_LIVE_CHECK_JQ.to_owned()
        };

        let jq_result = jq::execute_jq(
            &serde_json::to_value(live_checker).map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?,
            &jq_filter,
            &BTreeMap::new(),
        )
        .map_err(|e| Error::AdviceError {
            error: e.to_string(),
        })?;

        engine
            .add_data(&jq_result)
            .map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?;

        Ok(RegoAdvisor { engine })
    }

    fn check<T>(&mut self, input: T) -> Result<Vec<PolicyFinding>, Error>
    where
        T: Serialize,
    {
        self.engine
            .set_input(&input)
            .map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?;
        let violations = self
            .engine
            .check(weaver_checker::PolicyStage::LiveCheckAdvice)
            .map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?;
        // Extract advice from violations
        Ok(violations)
    }
}

/// Input data for the check function
#[derive(Serialize)]
struct RegoInput<'a> {
    sample: SampleRef<'a>,
    resource: Option<&'a SampleResource>,
    instrumentation_scope: Option<&'a SampleInstrumentationScope>,
    registry_attribute: Option<Rc<VersionedAttribute>>,
    registry_group: Option<Rc<VersionedSignal>>,
}

impl Advisor for RegoAdvisor {
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        signal: &Sample,
        registry_attribute: Option<Rc<VersionedAttribute>>,
        registry_group: Option<Rc<VersionedSignal>>,
        otlp_emitter: Option<Rc<OtlpEmitter>>,
    ) -> Result<Vec<PolicyFinding>, Error> {
        let mut findings = self.check(RegoInput {
            sample: sample.clone(),
            registry_attribute,
            resource: signal.resource(),
            instrumentation_scope: signal.instrumentation_scope(),
            registry_group,
        })?;

        // Populate signal_type and signal_name from the parent signal if not already set
        for finding in &mut findings {
            if finding.signal_type.is_none() {
                finding.signal_type = signal.signal_type();
            }
            if finding.signal_name.is_none() {
                finding.signal_name = signal.signal_name();
            }
        }

        // Emit each finding if emitter available
        emit_findings(&findings, &sample, otlp_emitter.as_deref(), signal);

        Ok(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_source_is_parsed_from_its_uri() {
        assert_eq!(
            AdvicePolicySource::builtin(BUILTIN_ADVICE_POLICIES).expect("a known built-in"),
            Some(AdvicePolicySource::Builtin)
        );
        assert_eq!(
            AdvicePolicySource::builtin("policies").expect("not a built-in"),
            None
        );
        assert!(AdvicePolicySource::builtin("builtin://unknown.rego").is_err());
    }
}
