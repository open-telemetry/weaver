// SPDX-License-Identifier: Apache-2.0

//! Stability advisor

use serde_json::json;
use std::rc::Rc;
use weaver_checker::{FindingLevel, PolicyFinding};
use weaver_semconv::stability::Stability;

use super::{Advisor, FindingBuilder};
use crate::{
    otlp_logger::OtlpEmitter, Error, FindingId, Sample, SampleRef, VersionedAttribute,
    VersionedSignal, ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY, EVENT_NAME_ADVICE_CONTEXT_KEY,
    METRIC_NAME_ADVICE_CONTEXT_KEY, STABILITY_ADVICE_CONTEXT_KEY,
};

/// An advisor that checks if a sample is stable from the stability field in the semantic convention
/// The value will be the stability level
pub struct StabilityAdvisor;

impl Advisor for StabilityAdvisor {
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        parent_signal: &Sample,
        registry_attribute: Option<Rc<VersionedAttribute>>,
        registry_group: Option<Rc<VersionedSignal>>,
        otlp_emitter: Option<Rc<OtlpEmitter>>,
    ) -> Result<Vec<PolicyFinding>, Error> {
        match sample {
            SampleRef::Attribute(sample_attribute) => {
                let mut findings = Vec::new();
                if let Some(attribute) = registry_attribute {
                    match attribute.stability() {
                        Some(ref stability) if *stability != &Stability::Stable => {
                            let name = &sample_attribute.name;
                            let finding = FindingBuilder::new(FindingId::NotStable)
                                .context(json!({
                                    ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY: name,
                                    STABILITY_ADVICE_CONTEXT_KEY: stability,
                                }))
                                .message(format!(
                                    "Attribute '{}' is not stable; stability = {}.",
                                    name, stability
                                ))
                                .level(FindingLevel::Improvement)
                                .signal(parent_signal)
                                .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                            findings.push(finding);
                        }
                        _ => {}
                    }
                }
                Ok(findings)
            }
            SampleRef::Metric(sample_metric) => {
                let mut findings = Vec::new();
                if let Some(group) = registry_group {
                    match group.stability() {
                        Some(ref stability) if *stability != &Stability::Stable => {
                            let name = &sample_metric.name;
                            let finding = FindingBuilder::new(FindingId::NotStable)
                                .context(json!({
                                    METRIC_NAME_ADVICE_CONTEXT_KEY: name,
                                    STABILITY_ADVICE_CONTEXT_KEY: stability,
                                }))
                                .message(format!(
                                    "Metric '{}' is not stable; stability = {stability}.",
                                    name
                                ))
                                .level(FindingLevel::Improvement)
                                .signal(parent_signal)
                                .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                            findings.push(finding);
                        }
                        _ => {}
                    }
                }
                Ok(findings)
            }
            SampleRef::Log(sample_log) => {
                let mut findings = Vec::new();
                if let Some(group) = registry_group {
                    match group.stability() {
                        Some(ref stability) if *stability != &Stability::Stable => {
                            let name = &sample_log.event_name;
                            let finding = FindingBuilder::new(FindingId::NotStable)
                                .context(json!({
                                    EVENT_NAME_ADVICE_CONTEXT_KEY: name,
                                    STABILITY_ADVICE_CONTEXT_KEY: stability,
                                }))
                                .message(format!(
                                    "Event '{}' is not stable; stability = {stability}.",
                                    name
                                ))
                                .level(FindingLevel::Improvement)
                                .signal(parent_signal)
                                .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                            findings.push(finding);
                        }
                        _ => {}
                    }
                }
                Ok(findings)
            }
            _ => Ok(Vec::new()),
        }
    }
}
