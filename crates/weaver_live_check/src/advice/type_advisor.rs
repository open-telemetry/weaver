// SPDX-License-Identifier: Apache-2.0

//! Type validation and required attribute checking advisor

use serde_json::json;
use std::{collections::HashSet, rc::Rc};
use weaver_checker::{FindingLevel, PolicyFinding};
use weaver_forge::v2::{event::EventAttribute, metric::MetricAttribute};
use weaver_resolved_schema::attribute::Attribute;
use weaver_semconv::attribute::{
    AttributeType, BasicRequirementLevelSpec, PrimitiveOrArrayTypeSpec, RequirementLevel,
    TemplateTypeSpec,
};

use weaver_semconv::entity_association::EntityAssociation;

use super::{emit_findings, Advisor, FindingBuilder};
use crate::{
    live_checker::LiveChecker, otlp_logger::OtlpEmitter, sample_attribute::SampleAttribute,
    sample_metric::SampleInstrument, Error, FindingId, Sample, SampleRef, VersionedAttribute,
    VersionedEntity, VersionedSignal, ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY,
    ATTRIBUTE_TYPE_ADVICE_CONTEXT_KEY, ENTITY_TYPE_ADVICE_CONTEXT_KEY,
    EXPECTED_VALUE_ADVICE_CONTEXT_KEY, INSTRUMENT_ADVICE_CONTEXT_KEY, UNIT_ADVICE_CONTEXT_KEY,
};

/// An advisor that checks if a sample has the correct type
pub struct TypeAdvisor;

/// Trait to abstract over different attribute types for checking
trait CheckableAttribute {
    fn key(&self) -> &str;
    fn requirement_level(&self) -> &RequirementLevel;
    fn attribute_type(&self) -> &AttributeType;
}

impl CheckableAttribute for Attribute {
    fn key(&self) -> &str {
        &self.name
    }

    fn requirement_level(&self) -> &RequirementLevel {
        &self.requirement_level
    }

    fn attribute_type(&self) -> &AttributeType {
        &self.r#type
    }
}

impl CheckableAttribute for MetricAttribute {
    fn key(&self) -> &str {
        &self.base.key
    }

    fn requirement_level(&self) -> &RequirementLevel {
        &self.requirement_level
    }

    fn attribute_type(&self) -> &AttributeType {
        &self.base.r#type
    }
}

impl CheckableAttribute for EventAttribute {
    fn key(&self) -> &str {
        &self.base.key
    }

    fn requirement_level(&self) -> &RequirementLevel {
        &self.requirement_level
    }

    fn attribute_type(&self) -> &AttributeType {
        &self.base.r#type
    }
}

/// Checks resource attributes against an entity definition's requirements.
///
/// Findings use entity-specific `FindingId` variants and include `entity_type` in context.
pub(crate) fn check_entity_resource_attributes(
    entity: &VersionedEntity,
    resource_attributes: &[SampleAttribute],
    parent_signal: &Sample,
) -> Vec<PolicyFinding> {
    let attribute_set: HashSet<_> = resource_attributes
        .iter()
        .map(|a| a.name.as_str())
        .collect();

    let mut advice_list = Vec::new();

    let check_attr = |key: &str,
                      requirement_level: &RequirementLevel,
                      entity_type: &str,
                      advice_list: &mut Vec<PolicyFinding>| {
        if attribute_set.contains(key) {
            return;
        }
        let (finding_id, advice_level, message) = match requirement_level {
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required) => (
                FindingId::EntityRequiredAttributeNotPresent,
                FindingLevel::Violation,
                format!("Required attribute '{key}' for entity '{entity_type}' is not present in the resource."),
            ),
            RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended)
            | RequirementLevel::Recommended { .. } => (
                FindingId::EntityRecommendedAttributeNotPresent,
                FindingLevel::Improvement,
                format!("Recommended attribute '{key}' for entity '{entity_type}' is not present in the resource."),
            ),
            RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn)
            | RequirementLevel::OptIn { .. } => (
                FindingId::EntityOptInAttributeNotPresent,
                FindingLevel::Information,
                format!("Opt-in attribute '{key}' for entity '{entity_type}' is not present in the resource."),
            ),
            RequirementLevel::ConditionallyRequired { .. } => (
                FindingId::EntityConditionallyRequiredAttributeNotPresent,
                FindingLevel::Information,
                format!("Conditionally required attribute '{key}' for entity '{entity_type}' is not present in the resource."),
            ),
        };
        advice_list.push(PolicyFinding {
            id: finding_id.into(),
            context: Some(json!({
                ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY: key,
                ENTITY_TYPE_ADVICE_CONTEXT_KEY: entity_type,
            })),
            message,
            level: advice_level,
            signal_type: parent_signal.signal_type(),
            signal_name: parent_signal.signal_name(),
        });
    };

    match entity {
        VersionedEntity::V1(group) => {
            let entity_type = group.name.as_deref().unwrap_or("");
            for attr in &group.attributes {
                check_attr(
                    &attr.name,
                    &attr.requirement_level,
                    entity_type,
                    &mut advice_list,
                );
            }
        }
        VersionedEntity::V2(entity) => {
            let entity_type = entity.r#type.to_string();
            for attr in entity.identity.iter().chain(entity.description.iter()) {
                check_attr(
                    &attr.base.key,
                    &attr.requirement_level,
                    &entity_type,
                    &mut advice_list,
                );
            }
        }
    }

    advice_list
}

/// The outcome of evaluating an entity association expression against the resource.
struct AssocEval {
    /// Whether the expression is satisfied (all required attributes of the chosen path present).
    satisfied: bool,
    /// Findings to surface to the user for the chosen path.
    findings: Vec<PolicyFinding>,
}

/// Evaluates a signal's entity associations against the resource and returns the findings to emit.
///
/// The top-level list is an implicit `one_of`: the telemetry must satisfy at least one entry.
/// `one_of`/`all_of` combinators may be nested arbitrarily.
pub(crate) fn check_entity_associations(
    associations: &[EntityAssociation],
    live_checker: &LiveChecker,
    resource_attributes: &[SampleAttribute],
    parent_signal: &Sample,
) -> Vec<PolicyFinding> {
    match associations {
        [] => Vec::new(),
        // A single top-level entry is evaluated directly so a lone `all_of` keeps its detailed
        // per-attribute findings instead of being collapsed into an aggregate.
        [single] => {
            evaluate_association(single, live_checker, resource_attributes, parent_signal).findings
        }
        // Multiple top-level entries combine as an implicit `one_of`.
        many => evaluate_one_of(many, live_checker, resource_attributes, parent_signal).findings,
    }
}

/// Recursively evaluates a single entity association expression.
fn evaluate_association(
    assoc: &EntityAssociation,
    live_checker: &LiveChecker,
    resource_attributes: &[SampleAttribute],
    parent_signal: &Sample,
) -> AssocEval {
    match assoc {
        EntityAssociation::Ref(name) => match live_checker.find_entity(name) {
            Some(entity) => {
                let findings =
                    check_entity_resource_attributes(&entity, resource_attributes, parent_signal);
                // Satisfied when no required (Violation-level) attribute is missing. Any
                // remaining recommended/opt-in/conditional findings are surfaced as improvements.
                let satisfied = !findings.iter().any(|f| f.level == FindingLevel::Violation);
                AssocEval {
                    satisfied,
                    findings,
                }
            }
            // Unknown entity references are skipped (treated as neutral) to avoid spurious
            // violations for entities that are not present in the registry.
            None => AssocEval {
                satisfied: true,
                findings: Vec::new(),
            },
        },
        EntityAssociation::OneOf { one_of } => {
            evaluate_one_of(one_of, live_checker, resource_attributes, parent_signal)
        }
        EntityAssociation::AllOf { all_of } => {
            evaluate_all_of(all_of, live_checker, resource_attributes, parent_signal)
        }
    }
}

/// Evaluates an `all_of` group: every child must be satisfied; all child findings are surfaced.
fn evaluate_all_of(
    children: &[EntityAssociation],
    live_checker: &LiveChecker,
    resource_attributes: &[SampleAttribute],
    parent_signal: &Sample,
) -> AssocEval {
    let mut satisfied = true;
    let mut findings = Vec::new();
    for child in children {
        let eval = evaluate_association(child, live_checker, resource_attributes, parent_signal);
        satisfied &= eval.satisfied;
        findings.extend(eval.findings);
    }
    AssocEval {
        satisfied,
        findings,
    }
}

/// Evaluates a `one_of` group: at least one child must be satisfied. When satisfied, only the
/// satisfied branches' improvement findings are surfaced; when none are satisfied, a single
/// aggregate finding naming the candidate entities is emitted.
fn evaluate_one_of(
    children: &[EntityAssociation],
    live_checker: &LiveChecker,
    resource_attributes: &[SampleAttribute],
    parent_signal: &Sample,
) -> AssocEval {
    let mut any_satisfied = false;
    let mut findings = Vec::new();
    for child in children {
        let eval = evaluate_association(child, live_checker, resource_attributes, parent_signal);
        if eval.satisfied {
            any_satisfied = true;
            findings.extend(eval.findings);
        }
    }
    if any_satisfied {
        AssocEval {
            satisfied: true,
            findings,
        }
    } else {
        AssocEval {
            satisfied: false,
            findings: vec![entity_association_not_satisfied(children, parent_signal)],
        }
    }
}

/// Builds the aggregate finding emitted when no branch of a `one_of` group is satisfied.
fn entity_association_not_satisfied(
    children: &[EntityAssociation],
    parent_signal: &Sample,
) -> PolicyFinding {
    // Collect the candidate entity names, de-duplicated while preserving order.
    let mut seen = HashSet::new();
    let entities: Vec<&str> = children
        .iter()
        .flat_map(EntityAssociation::referenced_entities)
        .filter(|name| seen.insert(*name))
        .collect();
    let message = format!(
        "None of the associated entities [{}] were satisfied by the resource.",
        entities.join(", ")
    );
    PolicyFinding {
        id: FindingId::EntityAssociationNotSatisfied.into(),
        context: Some(json!({
            ENTITY_TYPE_ADVICE_CONTEXT_KEY: entities,
        })),
        message,
        level: FindingLevel::Violation,
        signal_type: parent_signal.signal_type(),
        signal_name: parent_signal.signal_name(),
    }
}

/// Checks if attributes from a resolved group are present in a list of sample attributes
///
/// Returns a list of advice for the attributes based on their RequirementLevel.
///
/// If an attribute is not present in the sample:
///
/// | RequirementLevel       | Live-check advice level |
/// |------------------------|-------------------------|
/// | Required               | Violation               |
/// | Recommended            | Improvement             |
/// | Opt-In                 | Information             |
/// | Conditionally Required | Information             |
fn check_attributes<T: CheckableAttribute>(
    semconv_attributes: &[T],
    sample_attributes: &[SampleAttribute],
    sample: &Sample,
) -> Vec<PolicyFinding> {
    // Create a HashSet of attribute names for O(1) lookups
    let attribute_set: HashSet<_> = sample_attributes
        .iter()
        .map(|attr| attr.name.as_str())
        .collect();

    let mut advice_list = Vec::new();
    for semconv_attribute in semconv_attributes {
        let key = semconv_attribute.key();
        // Check if this is a template attribute
        let is_template = matches!(
            semconv_attribute.attribute_type(),
            AttributeType::Template(_)
        );

        // For template attributes, check if any sample attribute starts with the template prefix
        // For non-template attributes, check for exact match
        let is_present = if is_template {
            sample_attributes
                .iter()
                .any(|attr| attr.name.starts_with(key))
        } else {
            attribute_set.contains(key)
        };

        if !is_present {
            let (finding_id, advice_level, message) = match semconv_attribute.requirement_level() {
                RequirementLevel::Basic(BasicRequirementLevelSpec::Required) => (
                    FindingId::RequiredAttributeNotPresent,
                    FindingLevel::Violation,
                    format!("Required attribute '{key}' is not present."),
                ),
                RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended)
                | RequirementLevel::Recommended { .. } => (
                    FindingId::RecommendedAttributeNotPresent,
                    FindingLevel::Improvement,
                    format!("Recommended attribute '{key}' is not present."),
                ),
                RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn)
                | RequirementLevel::OptIn { .. } => (
                    FindingId::OptInAttributeNotPresent,
                    FindingLevel::Information,
                    format!("Opt-in attribute '{key}' is not present."),
                ),
                RequirementLevel::ConditionallyRequired { .. } => (
                    FindingId::ConditionallyRequiredAttributeNotPresent,
                    FindingLevel::Information,
                    format!("Conditionally required attribute '{key}' is not present."),
                ),
            };
            advice_list.push(PolicyFinding {
                id: finding_id.into(),
                context: Some(json!({
                    ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY: key.to_owned()
                })),
                message,
                level: advice_level,
                signal_type: sample.signal_type(),
                signal_name: sample.signal_name(),
            });
        }
    }
    advice_list
}

impl Advisor for TypeAdvisor {
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
                // Only provide advice if the attribute is a match and the type is present
                match (registry_attribute, sample_attribute.r#type.as_ref()) {
                    (Some(semconv_attribute), Some(attribute_type)) => {
                        let semconv_attribute_type = match &semconv_attribute.r#type() {
                            AttributeType::PrimitiveOrArray(primitive_or_array_type_spec) => {
                                primitive_or_array_type_spec
                            }
                            AttributeType::Template(template_type_spec) => {
                                &match template_type_spec {
                                    TemplateTypeSpec::Boolean => PrimitiveOrArrayTypeSpec::Boolean,
                                    TemplateTypeSpec::Int => PrimitiveOrArrayTypeSpec::Int,
                                    TemplateTypeSpec::Double => PrimitiveOrArrayTypeSpec::Double,
                                    TemplateTypeSpec::String => PrimitiveOrArrayTypeSpec::String,
                                    TemplateTypeSpec::Any => PrimitiveOrArrayTypeSpec::Any,
                                    TemplateTypeSpec::Strings => PrimitiveOrArrayTypeSpec::Strings,
                                    TemplateTypeSpec::Ints => PrimitiveOrArrayTypeSpec::Ints,
                                    TemplateTypeSpec::Doubles => PrimitiveOrArrayTypeSpec::Doubles,
                                    TemplateTypeSpec::Booleans => {
                                        PrimitiveOrArrayTypeSpec::Booleans
                                    }
                                }
                            }
                            AttributeType::Enum { .. } => {
                                // Special case: Enum variants can be either string or int
                                if attribute_type != &PrimitiveOrArrayTypeSpec::String
                                    && attribute_type != &PrimitiveOrArrayTypeSpec::Int
                                {
                                    let name = &sample_attribute.name;
                                    let finding = FindingBuilder::new(FindingId::TypeMismatch)
                                        .context(json!({
                                            ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY: name,
                                            ATTRIBUTE_TYPE_ADVICE_CONTEXT_KEY: attribute_type,
                                        }))
                                        .message(format!(
                                            "Enum attribute '{}' has type '{}'. Enum value type should be 'string' or 'int'.",
                                            name, attribute_type
                                        ))
                                        .level(FindingLevel::Violation)
                                        .signal(parent_signal)
                                        .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                                    return Ok(vec![finding]);
                                } else {
                                    return Ok(Vec::new());
                                }
                            }
                        };

                        if !attribute_type.is_compatible(semconv_attribute_type) {
                            let name = &sample_attribute.name;
                            let finding = FindingBuilder::new(FindingId::TypeMismatch)
                                .context(json!({
                                    ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY: name,
                                    ATTRIBUTE_TYPE_ADVICE_CONTEXT_KEY: attribute_type,
                                    EXPECTED_VALUE_ADVICE_CONTEXT_KEY: semconv_attribute_type,
                                }))
                                .message(format!(
                                    "Attribute '{}' has type '{}'. Type should be '{}'.",
                                    name, attribute_type, semconv_attribute_type
                                ))
                                .level(FindingLevel::Violation)
                                .signal(parent_signal)
                                .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                            Ok(vec![finding])
                        } else {
                            Ok(Vec::new())
                        }
                    }
                    _ => Ok(Vec::new()),
                }
            }
            SampleRef::Metric(sample_metric) => {
                // Check the instrument and unit of the metric
                let mut advice_list = Vec::new();

                if let Some(semconv_metric) = registry_group {
                    match &sample_metric.instrument {
                        SampleInstrument::Unsupported(name) => {
                            let finding = FindingBuilder::new(FindingId::UnexpectedInstrument)
                                .context(json!({
                                    INSTRUMENT_ADVICE_CONTEXT_KEY: name,
                                }))
                                .message(format!("Instrument '{name}' is not supported"))
                                .level(FindingLevel::Violation)
                                .signal(parent_signal)
                                .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                            advice_list.push(finding);
                        }
                        SampleInstrument::Supported(sample_instrument) => {
                            if let Some(semconv_instrument) = semconv_metric.instrument() {
                                if semconv_instrument != sample_instrument {
                                    let finding = FindingBuilder::new(FindingId::UnexpectedInstrument)
                                        .context(json!({
                                            INSTRUMENT_ADVICE_CONTEXT_KEY: sample_instrument,
                                            EXPECTED_VALUE_ADVICE_CONTEXT_KEY: semconv_instrument,
                                        }))
                                        .message(format!(
                                            "Instrument should be '{semconv_instrument}', but found '{sample_instrument}'."
                                        ))
                                        .level(FindingLevel::Violation)
                                        .signal(parent_signal)
                                        .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                                    advice_list.push(finding);
                                }
                            }
                        }
                    }

                    if let Some(semconv_unit) = semconv_metric.unit() {
                        if semconv_unit != &sample_metric.unit {
                            let unit = &sample_metric.unit;
                            let finding = FindingBuilder::new(FindingId::UnitMismatch)
                                .context(json!({
                                    UNIT_ADVICE_CONTEXT_KEY: unit,
                                    EXPECTED_VALUE_ADVICE_CONTEXT_KEY: semconv_unit,
                                }))
                                .message(format!(
                                    "Unit should be '{semconv_unit}', but found '{unit}'."
                                ))
                                .level(FindingLevel::Violation)
                                .signal(parent_signal)
                                .build_and_emit(&sample, otlp_emitter.as_deref(), parent_signal);

                            advice_list.push(finding);
                        }
                    }
                }
                Ok(advice_list)
            }
            SampleRef::NumberDataPoint(sample_number_data_point) => {
                if let Some(semconv_metric) = registry_group {
                    let advice_list = match &*semconv_metric {
                        VersionedSignal::Group(group) => check_attributes(
                            &group.attributes,
                            &sample_number_data_point.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Metric(metric) => check_attributes(
                            &metric.attributes,
                            &sample_number_data_point.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Span(_span) => vec![],
                        VersionedSignal::Event(_event) => vec![],
                    };

                    // Emit each finding if emitter available
                    emit_findings(
                        &advice_list,
                        &sample,
                        otlp_emitter.as_deref(),
                        parent_signal,
                    );

                    Ok(advice_list)
                } else {
                    Ok(Vec::new())
                }
            }
            SampleRef::HistogramDataPoint(sample_histogram_data_point) => {
                if let Some(semconv_metric) = registry_group {
                    let advice_list = match &*semconv_metric {
                        VersionedSignal::Group(group) => check_attributes(
                            &group.attributes,
                            &sample_histogram_data_point.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Metric(metric) => check_attributes(
                            &metric.attributes,
                            &sample_histogram_data_point.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Span(_span) => vec![],
                        VersionedSignal::Event(_event) => vec![],
                    };

                    // Emit each finding if emitter available
                    emit_findings(
                        &advice_list,
                        &sample,
                        otlp_emitter.as_deref(),
                        parent_signal,
                    );

                    Ok(advice_list)
                } else {
                    Ok(Vec::new())
                }
            }
            SampleRef::Log(sample_log) => {
                if let Some(semconv_event) = registry_group {
                    let advice_list = match &*semconv_event {
                        VersionedSignal::Group(group) => check_attributes(
                            &group.attributes,
                            &sample_log.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Event(event) => check_attributes(
                            &event.attributes,
                            &sample_log.attributes,
                            parent_signal,
                        ),
                        VersionedSignal::Span(_span) => vec![],
                        VersionedSignal::Metric(_metric) => vec![],
                    };

                    // Emit each finding if emitter available
                    emit_findings(
                        &advice_list,
                        &sample,
                        otlp_emitter.as_deref(),
                        parent_signal,
                    );

                    Ok(advice_list)
                } else {
                    Ok(Vec::new())
                }
            }
            _ => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::sample_attribute::SampleAttribute;
    use crate::sample_metric::{SampleInstrument, SampleMetric};
    use weaver_checker::FindingLevel;
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::attribute::{
        AttributeType::PrimitiveOrArray, BasicRequirementLevelSpec, PrimitiveOrArrayTypeSpec,
        RequirementLevel,
    };

    fn create_test_attribute(name: &str, requirement_level: RequirementLevel) -> Attribute {
        Attribute {
            name: name.to_owned(),
            requirement_level,
            r#type: PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: "test attribute".to_owned(),
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
        }
    }

    fn create_sample_attribute(name: &str) -> SampleAttribute {
        SampleAttribute {
            name: name.to_owned(),
            value: None,
            r#type: None,
            live_check_result: None,
        }
    }

    #[test]
    fn test_check_attributes_all_requirement_levels() {
        let semconv_attributes = vec![
            create_test_attribute(
                "required_attr",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            ),
            create_test_attribute(
                "recommended_basic",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            ),
            create_test_attribute(
                "recommended_text",
                RequirementLevel::Recommended {
                    text: "This is recommended".to_owned(),
                },
            ),
            create_test_attribute(
                "opt_in_basic",
                RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn),
            ),
            create_test_attribute(
                "opt_in_text",
                RequirementLevel::OptIn {
                    text: "This is opt-in".to_owned(),
                },
            ),
            create_test_attribute(
                "conditional",
                RequirementLevel::ConditionallyRequired {
                    text: "Required when X".to_owned(),
                },
            ),
        ];

        // Provide no attributes
        let sample_attributes = vec![];

        // Use a dummy Sample for signal_type and signal_name
        let sample = Sample::Metric(SampleMetric {
            name: "test_metric".to_owned(),
            unit: "".to_owned(),
            data_points: None,
            instrument: SampleInstrument::Supported(weaver_semconv::group::InstrumentSpec::Counter),
            live_check_result: None,
            resource: None,
        });

        let advice = check_attributes(&semconv_attributes, &sample_attributes, &sample);
        assert_eq!(advice.len(), 6);

        // Verify each advice type and level
        let advice_map: HashMap<_, _> = advice.iter().map(|a| (a.id.clone(), a.level)).collect();

        assert_eq!(
            advice_map.get("recommended_attribute_not_present"),
            Some(&FindingLevel::Improvement)
        );
        assert_eq!(
            advice_map.get("opt_in_attribute_not_present"),
            Some(&FindingLevel::Information)
        );
        assert_eq!(
            advice_map.get("conditionally_required_attribute_not_present"),
            Some(&FindingLevel::Information)
        );
        assert_eq!(
            advice_map.get("required_attribute_not_present"),
            Some(&FindingLevel::Violation)
        );

        // Count advice levels
        let violations = advice
            .iter()
            .filter(|a| a.level == FindingLevel::Violation)
            .count();
        let improvements = advice
            .iter()
            .filter(|a| a.level == FindingLevel::Improvement)
            .count();
        let information = advice
            .iter()
            .filter(|a| a.level == FindingLevel::Information)
            .count();

        assert_eq!(violations, 1);
        assert_eq!(improvements, 2);
        assert_eq!(information, 3);
    }

    #[test]
    fn test_check_attributes_no_missing_attributes() {
        let semconv_attributes = vec![
            create_test_attribute(
                "attr1",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            ),
            create_test_attribute(
                "attr2",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            ),
        ];
        let sample_attributes = vec![
            create_sample_attribute("attr1"),
            create_sample_attribute("attr2"),
        ];

        // Use a dummy Sample for signal_type and signal_name
        let sample = Sample::Metric(SampleMetric {
            name: "test_metric".to_owned(),
            unit: "".to_owned(),
            data_points: None,
            instrument: SampleInstrument::Supported(weaver_semconv::group::InstrumentSpec::Counter),
            live_check_result: None,
            resource: None,
        });
        let advice = check_attributes(&semconv_attributes, &sample_attributes, &sample);
        assert!(advice.is_empty());
    }

    #[test]
    fn test_check_attributes_template_type() {
        use weaver_semconv::attribute::{AttributeType, TemplateTypeSpec};

        // Create a template attribute like "weaver.finding.context"
        let template_attribute = Attribute {
            name: "weaver.finding.context".to_owned(),
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            r#type: AttributeType::Template(TemplateTypeSpec::Any),
            brief: "Template attribute for context".to_owned(),
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
        };

        let semconv_attributes = vec![template_attribute];

        // Test 1: Template attribute with matching prefix - should NOT generate advice
        let sample_attributes_with_match = vec![
            create_sample_attribute("weaver.finding.context.foo"),
            create_sample_attribute("weaver.finding.context.bar"),
        ];

        let sample = Sample::Metric(SampleMetric {
            name: "test_metric".to_owned(),
            unit: "".to_owned(),
            data_points: None,
            instrument: SampleInstrument::Supported(weaver_semconv::group::InstrumentSpec::Counter),
            live_check_result: None,
            resource: None,
        });

        let advice = check_attributes(&semconv_attributes, &sample_attributes_with_match, &sample);
        assert!(
            advice.is_empty(),
            "Expected no advice when template attribute has matching prefixed attributes"
        );

        // Test 2: Template attribute without matching prefix - SHOULD generate advice
        let sample_attributes_without_match = vec![
            create_sample_attribute("other.attribute"),
            create_sample_attribute("another.attribute"),
        ];

        let advice = check_attributes(
            &semconv_attributes,
            &sample_attributes_without_match,
            &sample,
        );
        assert_eq!(
            advice.len(),
            1,
            "Expected advice when template attribute has no matching prefixed attributes"
        );
        assert_eq!(advice[0].id, "recommended_attribute_not_present");
        assert_eq!(advice[0].level, FindingLevel::Improvement);

        // Test 3: Mix of template and non-template attributes
        let regular_attribute = create_test_attribute(
            "regular.attr",
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
        );
        let mixed_semconv_attributes = vec![
            Attribute {
                name: "template.attr".to_owned(),
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                r#type: AttributeType::Template(TemplateTypeSpec::String),
                brief: "Template attribute".to_owned(),
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
            },
            regular_attribute,
        ];

        let mixed_sample_attributes = vec![
            create_sample_attribute("template.attr.key1"), // Matches template
            create_sample_attribute("regular.attr"),       // Matches regular
        ];

        let advice = check_attributes(&mixed_semconv_attributes, &mixed_sample_attributes, &sample);
        assert!(
            advice.is_empty(),
            "Expected no advice when both template and regular attributes are present"
        );
    }
}
