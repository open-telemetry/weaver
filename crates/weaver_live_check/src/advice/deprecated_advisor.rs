// SPDX-License-Identifier: Apache-2.0

//! Deprecation detection advisor

use serde_json::json;
use std::rc::Rc;
use weaver_checker::{FindingLevel, PolicyFinding};
use weaver_semconv::deprecated::Deprecated;

use super::{Advisor, FindingBuilder};
use crate::{
    otlp_logger::OtlpEmitter, Error, FindingId, Sample, SampleRef, VersionedAttribute,
    VersionedSignal, ATTRIBUTE_NAME_ADVICE_CONTEXT_KEY, DEPRECATION_NOTE_ADVICE_CONTEXT_KEY,
    DEPRECATION_REASON_ADVICE_CONTEXT_KEY, EVENT_NAME_ADVICE_CONTEXT_KEY,
    METRIC_NAME_ADVICE_CONTEXT_KEY,
};

/// Convert a Deprecated value to a reason string
fn deprecated_to_reason(deprecated: &Deprecated) -> String {
    match deprecated {
        Deprecated::Renamed { .. } => "renamed".to_owned(),
        Deprecated::Obsoleted { .. } => "obsoleted".to_owned(),
        Deprecated::Uncategorized { .. } | Deprecated::Unspecified { .. } => {
            "uncategorized".to_owned()
        }
    }
}

/// Format a consistent deprecation message
fn format_deprecation_message(
    entity_type: &str,
    entity_name: &str,
    deprecated: &Deprecated,
) -> String {
    format!(
        "{} '{}' is deprecated; reason = '{}', note = '{}'.",
        entity_type,
        entity_name,
        deprecated_to_reason(deprecated),
        deprecated
    )
}

/// An advisor that checks if an attribute is deprecated
pub struct DeprecatedAdvisor;

impl Advisor for DeprecatedAdvisor {
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        signal: &Sample,
        registry_attribute: Option<Rc<VersionedAttribute>>,
        registry_group: Option<Rc<VersionedSignal>>,
        otlp_emitter: Option<Rc<OtlpEmitter>>,
    ) -> Result<Vec<PolicyFinding>, Error> {
        match sample {
            SampleRef::Attribute(sample_attribute) => {
                let mut findings = Vec::new();
                if let Some(attribute) = registry_attribute {
                    if let Some(deprecated) = &attribute.deprecated() {
                        let name = &sample_attribute.name;
                        let finding = FindingBuilder::new(FindingId::Deprecated)
                            .context(json!({
                                ATTRIBUTE_NAME_ADVICE_CONTEXT_KEY: name,
                                DEPRECATION_REASON_ADVICE_CONTEXT_KEY: deprecated_to_reason(deprecated),
                                DEPRECATION_NOTE_ADVICE_CONTEXT_KEY: deprecated.to_string(),
                            }))
                            .message(format_deprecation_message("Attribute", name, deprecated))
                            .level(FindingLevel::Violation)
                            .signal(signal)
                            .build_and_emit(&sample, otlp_emitter.as_deref(), signal);

                        findings.push(finding);
                    }
                }
                Ok(findings)
            }
            SampleRef::Metric(sample_metric) => {
                let mut findings = Vec::new();
                if let Some(group) = registry_group {
                    if let Some(deprecated) = &group.deprecated() {
                        let name = &sample_metric.name;
                        let finding = FindingBuilder::new(FindingId::Deprecated)
                            .context(json!({
                                METRIC_NAME_ADVICE_CONTEXT_KEY: name,
                                DEPRECATION_REASON_ADVICE_CONTEXT_KEY: deprecated_to_reason(deprecated),
                                DEPRECATION_NOTE_ADVICE_CONTEXT_KEY: deprecated.to_string(),
                            }))
                            .message(format_deprecation_message("Metric", name, deprecated))
                            .level(FindingLevel::Violation)
                            .signal(signal)
                            .build_and_emit(&sample, otlp_emitter.as_deref(), signal);

                        findings.push(finding);
                    }
                }
                Ok(findings)
            }
            SampleRef::Log(sample_log) => {
                let mut findings = Vec::new();
                if let Some(group) = registry_group {
                    if let Some(deprecated) = &group.deprecated() {
                        let name = &sample_log.event_name;
                        let finding = FindingBuilder::new(FindingId::Deprecated)
                            .context(json!({
                                EVENT_NAME_ADVICE_CONTEXT_KEY: name,
                                DEPRECATION_REASON_ADVICE_CONTEXT_KEY: deprecated_to_reason(deprecated),
                                DEPRECATION_NOTE_ADVICE_CONTEXT_KEY: deprecated.to_string(),
                            }))
                            .message(format_deprecation_message("Event", name, deprecated))
                            .level(FindingLevel::Violation)
                            .signal(signal)
                            .build_and_emit(&sample, otlp_emitter.as_deref(), signal);

                        findings.push(finding);
                    }
                }
                Ok(findings)
            }
            _ => Ok(Vec::new()),
        }
    }
}
