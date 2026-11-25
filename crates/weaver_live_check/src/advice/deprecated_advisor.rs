// SPDX-License-Identifier: Apache-2.0

//! Deprecation detection advisor

use serde_json::json;
use std::rc::Rc;
use weaver_checker::{FindingLevel, PolicyFinding};
use weaver_semconv::v2::deprecated::Deprecated;

use super::{Advisor, FindingBuilder};
use crate::{
    otlp_logger::OtlpEmitter, Error, FindingId, Sample, SampleRef, VersionedAttribute,
    VersionedSignal, ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY, DEPRECATED_ADVICE_CONTEXT_KEY,
    EVENT_NAME_ADVICE_CONTEXT_KEY, METRIC_NAME_ADVICE_CONTEXT_KEY,
};

fn deprecated_to_reason(deprecated: &Deprecated) -> &'static str {
    match deprecated {
        Deprecated::Renamed { .. } => "renamed",
        Deprecated::Obsoleted { .. } => "obsoleted",
        Deprecated::Uncategorized { .. } => "uncategorized",
    }
}

fn deprecation_context(name_key: &str, name: &str, deprecated: &Deprecated) -> serde_json::Value {
    json!({
        name_key: name,
        DEPRECATED_ADVICE_CONTEXT_KEY: deprecated,
    })
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
                            .context(deprecation_context(
                                ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY,
                                name,
                                deprecated,
                            ))
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
                            .context(deprecation_context(
                                METRIC_NAME_ADVICE_CONTEXT_KEY,
                                name,
                                deprecated,
                            ))
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
                            .context(deprecation_context(
                                EVENT_NAME_ADVICE_CONTEXT_KEY,
                                name,
                                deprecated,
                            ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renamed_without_note_context_and_message() {
        let deprecated = Deprecated::Renamed {
            renamed_to: "new.attr".to_owned(),
            note: None,
        };
        assert_eq!(
            deprecation_context(ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY, "old.attr", &deprecated),
            json!({
                "attribute_key": "old.attr",
                "deprecated": {"reason": "renamed", "renamed_to": "new.attr"},
            })
        );
        assert_eq!(
            format_deprecation_message("Attribute", "old.attr", &deprecated),
            "Attribute 'old.attr' is deprecated; reason = 'renamed', note = 'Replaced by `new.attr`.'."
        );
    }
}
