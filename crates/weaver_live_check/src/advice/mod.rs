// SPDX-License-Identifier: Apache-2.0

//! Builtin advisors

use serde_json::Value as JsonValue;
use std::rc::Rc;
use weaver_checker::{FindingLevel, PolicyFinding};

use crate::{
    otlp_logger::OtlpEmitter, Error, Sample, SampleRef, VersionedAttribute, VersionedSignal,
};

// Internal modules
mod deprecated_advisor;
mod enum_advisor;
mod rego_advisor;
mod stability_advisor;
mod type_advisor;

// Public re-exports
pub use deprecated_advisor::DeprecatedAdvisor;
pub use enum_advisor::EnumAdvisor;
pub use rego_advisor::RegoAdvisor;
pub use stability_advisor::StabilityAdvisor;
pub use type_advisor::TypeAdvisor;

/// Provides advice on a sample
pub trait Advisor {
    /// Provide advice on a sample
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        signal: &Sample,
        registry_attribute: Option<Rc<VersionedAttribute>>,
        registry_group: Option<Rc<VersionedSignal>>,
        otlp_emitter: Option<Rc<OtlpEmitter>>,
    ) -> Result<Vec<PolicyFinding>, Error>;
}

/// Fluent builder for creating PolicyFinding instances with automatic emission
pub struct FindingBuilder {
    id: String,
    context: JsonValue,
    message: String,
    level: FindingLevel,
    signal_type: Option<String>,
    signal_name: Option<String>,
}

impl FindingBuilder {
    /// Create a new FindingBuilder with the given advice type ID
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            context: JsonValue::Null,
            message: String::new(),
            level: FindingLevel::Information,
            signal_type: None,
            signal_name: None,
        }
    }

    /// Set the context JSON for this finding
    #[must_use]
    pub fn context(mut self, context: JsonValue) -> Self {
        self.context = context;
        self
    }

    /// Set the human-readable message for this finding
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Set the finding level
    #[must_use]
    pub fn level(mut self, level: FindingLevel) -> Self {
        self.level = level;
        self
    }

    /// Set signal_type and signal_name from the parent Sample
    #[must_use]
    pub fn signal(mut self, signal: &Sample) -> Self {
        self.signal_type = signal.signal_type();
        self.signal_name = signal.signal_name();
        self
    }

    /// Build the PolicyFinding
    #[must_use]
    pub fn build(self) -> PolicyFinding {
        PolicyFinding {
            id: self.id,
            context: self.context,
            message: self.message,
            level: self.level,
            signal_type: self.signal_type,
            signal_name: self.signal_name,
        }
    }

    /// Build the PolicyFinding and emit it if an emitter is available
    #[must_use]
    pub fn build_and_emit(
        self,
        sample: &SampleRef<'_>,
        emitter: Option<&OtlpEmitter>,
    ) -> PolicyFinding {
        let finding = self.build();
        if let Some(emitter) = emitter {
            emitter.emit_finding(&finding, sample);
        }
        finding
    }
}

/// Batch emit findings for a given sample
pub(crate) fn emit_findings(
    findings: &[PolicyFinding],
    sample: &SampleRef<'_>,
    emitter: Option<&OtlpEmitter>,
) {
    if let Some(emitter) = emitter {
        for finding in findings {
            emitter.emit_finding(finding, sample);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::otlp_logger::OtlpEmitter;
    use crate::sample_attribute::SampleAttribute;

    use super::*;
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::attribute::{
        AttributeType::PrimitiveOrArray, BasicRequirementLevelSpec, PrimitiveOrArrayTypeSpec,
        RequirementLevel,
    };
    use weaver_semconv::deprecated::Deprecated;
    use weaver_semconv::stability::Stability;

    fn create_sample_attribute(name: &str) -> SampleAttribute {
        SampleAttribute {
            name: name.to_owned(),
            value: None,
            r#type: None,
            live_check_result: None,
        }
    }

    #[test]
    fn test_advisors_with_otlp_emitter() {
        // Test that advisors work with an OTLP emitter to exercise emit_finding code paths
        let emitter = Some(Rc::new(OtlpEmitter::new_stdout()));

        // Test DeprecatedAdvisor
        let mut deprecated_advisor = DeprecatedAdvisor;
        let deprecated_attr = Rc::new(VersionedAttribute::V1(Box::new(Attribute {
            name: "deprecated.attr".to_owned(),
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            r#type: PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: "deprecated attribute".to_owned(),
            examples: None,
            tag: None,
            stability: None,
            deprecated: Some(Deprecated::Obsoleted {
                note: "Use new.attr instead".to_owned(),
            }),
            sampling_relevant: None,
            note: "".to_owned(),
            prefix: false,
            annotations: None,
            role: None,
            tags: None,
            value: None,
        })));

        let sample_attr = create_sample_attribute("deprecated.attr");
        let sample = Sample::Attribute(sample_attr.clone());
        let findings = deprecated_advisor
            .advise(
                SampleRef::Attribute(&sample_attr),
                &sample,
                Some(deprecated_attr.clone()),
                None,
                emitter.clone(),
            )
            .unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, crate::DEPRECATED_ADVICE_TYPE);

        // Test TypeAdvisor
        let mut type_advisor = TypeAdvisor;
        let int_attr = Rc::new(VersionedAttribute::V1(Box::new(Attribute {
            name: "int.attr".to_owned(),
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            r#type: PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Int),
            brief: "integer attribute".to_owned(),
            examples: None,
            tag: None,
            stability: None,
            deprecated: None,
            sampling_relevant: None,
            note: "".to_owned(),
            prefix: false,
            annotations: None,
            role: None,
            tags: None,
            value: None,
        })));

        let mut sample_attr = create_sample_attribute("int.attr");
        sample_attr.r#type = Some(PrimitiveOrArrayTypeSpec::String);
        sample_attr.value = Some(serde_json::json!("wrong_type"));
        let sample = Sample::Attribute(sample_attr.clone());

        let findings = type_advisor
            .advise(
                SampleRef::Attribute(&sample_attr),
                &sample,
                Some(int_attr),
                None,
                emitter.clone(),
            )
            .unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, crate::TYPE_MISMATCH_ADVICE_TYPE);

        // Test StabilityAdvisor
        let mut stability_advisor = StabilityAdvisor;
        let dev_attr = Rc::new(VersionedAttribute::V1(Box::new(Attribute {
            name: "dev.attr".to_owned(),
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            r#type: PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: "development attribute".to_owned(),
            examples: None,
            tag: None,
            stability: Some(Stability::Development),
            deprecated: None,
            sampling_relevant: None,
            note: "".to_owned(),
            prefix: false,
            annotations: None,
            role: None,
            tags: None,
            value: None,
        })));

        let sample_attr = create_sample_attribute("dev.attr");
        let sample = Sample::Attribute(sample_attr.clone());

        let findings = stability_advisor
            .advise(
                SampleRef::Attribute(&sample_attr),
                &sample,
                Some(dev_attr),
                None,
                emitter,
            )
            .unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, crate::NOT_STABLE_ADVICE_TYPE);
    }
}
