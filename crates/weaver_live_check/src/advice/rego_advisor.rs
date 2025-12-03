// SPDX-License-Identifier: Apache-2.0

//! Rego policy-based advisor

use std::{collections::BTreeMap, path::PathBuf, rc::Rc};
use serde::Serialize;
use weaver_checker::{Engine, PolicyFinding};
use weaver_forge::jq;

use super::{Advisor, emit_findings};
use crate::{
    live_checker::LiveChecker, otlp_logger::OtlpEmitter, Error, Sample, SampleRef,
    VersionedAttribute, VersionedSignal, DEFAULT_LIVE_CHECK_JQ, DEFAULT_LIVE_CHECK_REGO,
    DEFAULT_LIVE_CHECK_REGO_POLICY_PATH,
};

/// An advisor which runs a rego policy on the attribute
pub struct RegoAdvisor {
    engine: Engine,
}

impl RegoAdvisor {
    /// Create a new RegoAdvisor
    pub fn new(
        live_checker: &LiveChecker,
        policy_dir: &Option<PathBuf>,
        jq_preprocessor: &Option<PathBuf>,
    ) -> Result<Self, Error> {
        let mut engine = Engine::new();
        if let Some(path) = policy_dir {
            let _ = engine
                .add_policies(path, "*.rego")
                .map_err(|e| Error::AdviceError {
                    error: e.to_string(),
                })?;
        } else {
            let _ = engine
                .add_policy(DEFAULT_LIVE_CHECK_REGO_POLICY_PATH, DEFAULT_LIVE_CHECK_REGO)
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
        emit_findings(&findings, &sample, otlp_emitter.as_deref());

        Ok(findings)
    }
}
