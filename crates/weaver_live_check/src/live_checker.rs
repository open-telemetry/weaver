// SPDX-License-Identifier: Apache-2.0

//! Holds the registry, helper structs, and the advisors for the live check

use serde::Serialize;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use weaver_semconv::{attribute::AttributeType, group::GroupType};

use crate::{
    advice::Advisor,
    finding_modifier::FindingModifier,
    matcher::{Matchers, SampleMatch, SignalKind},
    otlp_logger::OtlpEmitter,
    Error, SampleType, VersionedAttribute, VersionedEntity, VersionedRegistry, VersionedSignal,
};
use weaver_cel::Bindings;
use weaver_config::live_check::MatcherConfig;
use weaver_forge::v2::attribute::Attribute as V2Attribute;
use weaver_forge::v2::attribute_group::AttributeGroup;
use weaver_forge::v2::entity::{Entity as V2Entity, EntityRef};

#[cfg(test)]
use crate::CumulativeStatistics;

/// Attributes of one signal or attribute group, keyed by attribute key.
type AttributeIndex = HashMap<String, Rc<VersionedAttribute>>;

/// Signal attributes, keyed by signal id and then by attribute key.
type RefinedAttributes = HashMap<String, AttributeIndex>;

/// The longest template attribute in `index` that `key` extends.
fn find_template_in(index: &AttributeIndex, key: &str) -> Option<Rc<VersionedAttribute>> {
    index
        .iter()
        .filter(|(name, attribute)| {
            matches!(attribute.r#type(), AttributeType::Template(_))
                && key.starts_with(name.as_str())
        })
        .max_by_key(|(name, _)| name.len())
        .map(|(_, attribute)| Rc::clone(attribute))
}

/// A base attribute definition, and the schema urls of every registry that
/// declares it.
#[derive(Debug, Clone)]
pub struct BaseAttribute {
    /// The definition from the first registry that declares it.
    pub attribute: Rc<VersionedAttribute>,
    /// The schema urls that declare it, this registry first.
    pub schema_urls: Vec<String>,
}

impl BaseAttribute {
    /// The schema urls, as `a, b`.
    #[must_use]
    pub fn schema_urls(&self) -> String {
        self.schema_urls.join(", ")
    }
}

/// Indexes a signal's attributes by key.
fn index_attributes<'a>(
    attributes: impl Iterator<Item = &'a V2Attribute>,
) -> HashMap<String, Rc<VersionedAttribute>> {
    attributes
        .map(|attribute| {
            (
                attribute.key.clone(),
                Rc::new(VersionedAttribute::V2(attribute.clone())),
            )
        })
        .collect()
}

/// Holds the registry, helper structs, and the advisors for the live check
#[derive(Serialize)]
pub struct LiveChecker {
    /// The resolved registry
    pub registry: Arc<VersionedRegistry>,
    semconv_attributes: HashMap<String, Rc<VersionedAttribute>>,
    semconv_templates: HashMap<String, Rc<VersionedAttribute>>,
    semconv_metrics: HashMap<String, Rc<VersionedSignal>>,
    semconv_events: HashMap<String, Rc<VersionedSignal>>,
    /// v2 spans keyed by type, and v2 attribute groups keyed by id. Both are
    /// empty for a v1 registry, which has neither.
    #[serde(skip)]
    semconv_spans: HashMap<String, Rc<VersionedSignal>>,
    #[serde(skip)]
    semconv_attribute_groups: HashMap<String, Rc<AttributeGroup>>,
    /// The attributes each v2 signal declares, which carry its refinements.
    /// Empty for a v1 registry.
    #[serde(skip)]
    refined_span_attributes: RefinedAttributes,
    #[serde(skip)]
    refined_metric_attributes: RefinedAttributes,
    #[serde(skip)]
    refined_event_attributes: RefinedAttributes,
    /// The attributes each v2 attribute group declares, keyed by the attribute
    /// group's id.
    #[serde(skip)]
    attribute_group_attributes: RefinedAttributes,
    /// The base attributes of this registry and its dependencies, keyed by
    /// attribute key. Empty unless `search_all_attributes` is called.
    #[serde(skip)]
    base_attributes: HashMap<String, BaseAttribute>,
    /// Whether `search_all_attributes` was called.
    #[serde(skip)]
    searching_all_attributes: bool,
    #[serde(skip)]
    semconv_entities: HashMap<String, VersionedEntity>,
    /// The advisors to run
    #[serde(skip)]
    pub advisors: Vec<Box<dyn Advisor>>,
    #[serde(skip)]
    templates_by_length: Vec<(String, Rc<VersionedAttribute>)>,
    /// Optional OTLP emitter for emitting findings as log records
    #[serde(skip)]
    pub otlp_emitter: Option<Rc<OtlpEmitter>>,
    /// Optional finding modifier for overriding/filtering findings
    #[serde(skip)]
    pub finding_modifier: Option<FindingModifier>,
    /// The configured matchers, compiled and checked against the registry
    #[serde(skip)]
    matchers: Matchers,
}

impl LiveChecker {
    #[must_use]
    /// Create a new LiveChecker
    pub fn new(registry: Arc<VersionedRegistry>, advisors: Vec<Box<dyn Advisor>>) -> Self {
        // Create a hashmap of attributes for quick lookup
        let mut semconv_attributes = HashMap::new();
        let mut semconv_templates = HashMap::new();
        let mut templates_by_length = Vec::new();
        // Hashmap of metrics by name
        let mut semconv_metrics = HashMap::new();
        // Hashmap of events by name
        let mut semconv_events = HashMap::new();
        // Hashmap of entities by type name
        let mut semconv_entities = HashMap::new();
        // Hashmap of v2 spans by type, and v2 attribute groups by id
        let mut semconv_spans = HashMap::new();
        let mut semconv_attribute_groups = HashMap::new();
        // The attributes each v2 signal declares, by signal id
        let mut refined_span_attributes = RefinedAttributes::new();
        let mut refined_metric_attributes = RefinedAttributes::new();
        let mut refined_event_attributes = RefinedAttributes::new();
        let mut attribute_group_attributes = RefinedAttributes::new();

        match registry.as_ref() {
            VersionedRegistry::V1(registry) => {
                for group in &registry.groups {
                    if group.r#type == GroupType::Metric {
                        if let Some(metric_name) = &group.metric_name {
                            let group_rc = Rc::new(VersionedSignal::Group(Box::new(group.clone())));
                            let _ = semconv_metrics.insert(metric_name.clone(), group_rc);
                        }
                    }
                    if group.r#type == GroupType::Event {
                        if let Some(event_name) = &group.name {
                            let group_rc = Rc::new(VersionedSignal::Group(Box::new(group.clone())));
                            let _ = semconv_events.insert(event_name.clone(), group_rc);
                        }
                    }
                    if group.r#type == GroupType::Entity {
                        if let Some(entity_name) = &group.name {
                            let _ = semconv_entities.insert(
                                entity_name.clone(),
                                VersionedEntity::V1(Box::new(group.clone())),
                            );
                        }
                    }
                    for attribute in &group.attributes {
                        let attribute_rc = Rc::new(VersionedAttribute::V1(attribute.clone()));
                        match attribute.r#type {
                            AttributeType::Template(_) => {
                                templates_by_length
                                    .push((attribute.name.clone(), attribute_rc.clone()));
                                let _ =
                                    semconv_templates.insert(attribute.name.clone(), attribute_rc);
                            }
                            _ => {
                                let _ =
                                    semconv_attributes.insert(attribute.name.clone(), attribute_rc);
                            }
                        }
                    }
                }
            }
            VersionedRegistry::V2(registry) => {
                for metric in &registry.registry.metrics {
                    let metric_name = metric.name.to_string();
                    let _ = refined_metric_attributes.insert(
                        metric_name.clone(),
                        index_attributes(metric.attributes.iter().map(|a| &a.base)),
                    );
                    let metric_rc = Rc::new(VersionedSignal::Metric(metric.clone()));
                    let _ = semconv_metrics.insert(metric_name, metric_rc);
                }
                for event in &registry.registry.events {
                    let event_name = event.name.to_string();
                    let _ = refined_event_attributes.insert(
                        event_name.clone(),
                        index_attributes(event.attributes.iter().map(|a| &a.base)),
                    );
                    let event_rc = Rc::new(VersionedSignal::Event(event.clone()));
                    let _ = semconv_events.insert(event_name, event_rc);
                }
                for span in &registry.registry.spans {
                    let span_type = span.r#type.to_string();
                    let _ = refined_span_attributes.insert(
                        span_type.clone(),
                        index_attributes(span.attributes.iter().map(|a| &a.base)),
                    );
                    let span_rc = Rc::new(VersionedSignal::Span(span.clone()));
                    let _ = semconv_spans.insert(span_type, span_rc);
                }
                for attribute_group in &registry.registry.attribute_groups {
                    let attribute_group_id = attribute_group.id.to_string();
                    let _ = attribute_group_attributes.insert(
                        attribute_group_id.clone(),
                        index_attributes(attribute_group.attributes.iter().map(|a| &a.base)),
                    );
                    let _ = semconv_attribute_groups
                        .insert(attribute_group_id, Rc::new(attribute_group.clone()));
                }
                for entity in &registry.registry.entities {
                    let entity_type = entity.r#type.to_string();
                    let _ = semconv_entities
                        .insert(entity_type, VersionedEntity::V2(Box::new(entity.clone())));
                }
                for attribute in &registry.registry.attributes {
                    let attribute_rc = Rc::new(VersionedAttribute::V2(attribute.clone()));
                    match &attribute.r#type {
                        AttributeType::Template(_) => {
                            templates_by_length.push((attribute.key.clone(), attribute_rc.clone()));
                            let _ = semconv_templates.insert(attribute.key.clone(), attribute_rc);
                        }
                        _ => {
                            let _ = semconv_attributes.insert(attribute.key.clone(), attribute_rc);
                        }
                    }
                }
            }
        }

        // Sort templates by name length in descending order
        templates_by_length.sort_by_key(|(b, _)| std::cmp::Reverse(b.len()));

        LiveChecker {
            registry,
            semconv_attributes,
            semconv_templates,
            semconv_metrics,
            semconv_events,
            semconv_spans,
            semconv_attribute_groups,
            refined_span_attributes,
            refined_metric_attributes,
            refined_event_attributes,
            attribute_group_attributes,
            base_attributes: HashMap::new(),
            searching_all_attributes: false,
            semconv_entities,
            advisors,
            templates_by_length,
            otlp_emitter: None,
            finding_modifier: None,
            matchers: Matchers::default(),
        }
    }

    /// Compile the configured matchers and check them against the registry
    ///
    /// # Errors
    ///
    /// Returns an error when a matcher does not compile, reads a variable its
    /// sample type does not have, or names something that is not in the
    /// registry. Matchers need a v2 registry.
    pub fn set_matchers(&mut self, configs: &[MatcherConfig]) -> Result<(), Error> {
        let matchers = Matchers::compile(configs)?;
        matchers.check_against(self)?;
        self.matchers = matchers;
        Ok(())
    }

    /// The configured matchers
    #[must_use]
    pub fn matchers(&self) -> &Matchers {
        &self.matchers
    }

    /// Find the signal a matcher's `signal` names for this sample type
    ///
    /// Returns `None` for a sample type that has no signal.
    #[must_use]
    pub fn find_signal(
        &self,
        signal: &str,
        sample_type: SampleType,
    ) -> Option<Rc<VersionedSignal>> {
        match SignalKind::for_sample_type(sample_type)? {
            SignalKind::SpanType => self.find_span(signal),
            SignalKind::EventName => self.find_event(signal),
            SignalKind::MetricName => self.find_metric(signal),
        }
    }

    /// Counts a match against the matchers that produced it
    pub fn record_match(&mut self, sample_match: &SampleMatch) {
        self.matchers.record_match(sample_match);
    }

    /// The signal and attribute groups to compare a sample with
    #[must_use]
    pub fn match_for(
        &self,
        sample_type: SampleType,
        bindings: &dyn Bindings,
        natural: Option<Rc<VersionedSignal>>,
    ) -> SampleMatch {
        self.matchers
            .match_for(sample_type, bindings, natural, self)
    }

    /// Add an advisor
    pub fn add_advisor(&mut self, advisor: Box<dyn Advisor>) {
        self.advisors.push(advisor);
    }

    /// Find an attribute in the registry
    #[must_use]
    pub fn find_attribute(&self, name: &str) -> Option<Rc<VersionedAttribute>> {
        self.semconv_attributes.get(name).map(Rc::clone)
    }

    /// Find a metric in the registry
    #[must_use]
    pub fn find_metric(&self, name: &str) -> Option<Rc<VersionedSignal>> {
        self.semconv_metrics.get(name).map(Rc::clone)
    }

    /// Find an event in the registry
    #[must_use]
    pub fn find_event(&self, name: &str) -> Option<Rc<VersionedSignal>> {
        self.semconv_events.get(name).map(Rc::clone)
    }

    /// Find a v2 signal's own copy of an attribute, which carries its
    /// refinements
    ///
    /// `None` for a v1 group, and for an attribute the signal does not declare.
    #[must_use]
    pub fn find_refined_attribute(
        &self,
        signal: &VersionedSignal,
        key: &str,
    ) -> Option<Rc<VersionedAttribute>> {
        self.refined_index(signal)?.get(key).map(Rc::clone)
    }

    /// Find a template attribute of a signal that this key extends
    #[must_use]
    pub fn find_refined_template(
        &self,
        signal: &VersionedSignal,
        key: &str,
    ) -> Option<Rc<VersionedAttribute>> {
        find_template_in(self.refined_index(signal)?, key)
    }

    /// The attributes a signal declares, keyed by attribute key
    fn refined_index(&self, signal: &VersionedSignal) -> Option<&AttributeIndex> {
        let (index, id) = match signal {
            VersionedSignal::Span(span) => (&self.refined_span_attributes, &*span.r#type),
            VersionedSignal::Metric(metric) => (&self.refined_metric_attributes, &*metric.name),
            VersionedSignal::Event(event) => (&self.refined_event_attributes, &*event.name),
            VersionedSignal::Group(_) => return None,
        };
        index.get(id)
    }

    /// Find an attribute in the base definitions of this registry and its
    /// dependencies
    ///
    /// Always `None` unless `search_all_attributes` was called.
    #[must_use]
    pub fn find_base_attribute(&self, key: &str) -> Option<&BaseAttribute> {
        self.base_attributes.get(key)
    }

    /// Whether the base definitions are being searched
    #[must_use]
    pub fn is_searching_all_attributes(&self) -> bool {
        self.searching_all_attributes
    }

    /// Index the base attributes of this registry and its dependencies
    ///
    /// # Errors
    ///
    /// Returns an error for a v1 registry, which has no dependencies to search.
    pub fn search_all_attributes(&mut self) -> Result<(), Error> {
        let VersionedRegistry::V2(registry) = self.registry.as_ref() else {
            return Err(Error::SearchAllAttributesRequiresV2Registry);
        };
        self.searching_all_attributes = true;
        // This registry first, then nearest first, so a definition here wins over
        // a dependency's and a direct dependency's over a transitive one's.
        let sources = std::iter::once((&registry.schema_url, &registry.registry)).chain(
            registry
                .dependencies_nearest_first()
                .into_iter()
                .map(|(url, dependency)| (url, &dependency.registry)),
        );
        for (url, source) in sources {
            let schema_url = url.to_string();
            for attribute in &source.attributes {
                let _ = self
                    .base_attributes
                    .entry(attribute.key.clone())
                    .and_modify(|held| held.schema_urls.push(schema_url.clone()))
                    .or_insert_with(|| BaseAttribute {
                        attribute: Rc::new(VersionedAttribute::V2(attribute.clone())),
                        schema_urls: vec![schema_url.clone()],
                    });
            }
        }
        Ok(())
    }

    /// Find an attribute that a v2 attribute group declares
    #[must_use]
    pub fn find_attribute_group_attribute(
        &self,
        attribute_group_id: &str,
        key: &str,
    ) -> Option<Rc<VersionedAttribute>> {
        self.attribute_group_attributes
            .get(attribute_group_id)?
            .get(key)
            .map(Rc::clone)
    }

    /// Find a template attribute of a v2 attribute group that this key extends
    #[must_use]
    pub fn find_attribute_group_template(
        &self,
        attribute_group_id: &str,
        key: &str,
    ) -> Option<Rc<VersionedAttribute>> {
        find_template_in(
            self.attribute_group_attributes.get(attribute_group_id)?,
            key,
        )
    }

    /// Find a span in the registry by its type
    ///
    /// Always `None` for a v1 registry, which has no span types.
    #[must_use]
    pub fn find_span(&self, span_type: &str) -> Option<Rc<VersionedSignal>> {
        self.semconv_spans.get(span_type).map(Rc::clone)
    }

    /// Find an attribute group in the registry by its id
    ///
    /// Always `None` for a v1 registry, which has no attribute groups.
    #[must_use]
    pub fn find_attribute_group(&self, id: &str) -> Option<Rc<AttributeGroup>> {
        self.semconv_attribute_groups.get(id).map(Rc::clone)
    }

    /// Find an entity in the registry by type name
    ///
    /// The index holds the entities of this registry alone. A v2 association goes
    /// through [`Self::lookup_entity`] instead, which reads the registry that the
    /// reference names.
    #[must_use]
    pub fn find_entity(&self, entity_type: &str) -> Option<&VersionedEntity> {
        self.semconv_entities.get(entity_type)
    }

    /// Find the entity that a v2 association reference names
    ///
    /// The reference says which registry defines the entity, so this reads that
    /// one registry. `None` means no registry in scope answers the reference, or
    /// the registry under check is v1 and holds no such reference.
    #[must_use]
    pub fn lookup_entity(&self, entity_ref: &EntityRef) -> Option<&V2Entity> {
        match self.registry.as_ref() {
            VersionedRegistry::V2(registry) => registry.lookup_entity(entity_ref).ok(),
            VersionedRegistry::V1(_) => None,
        }
    }

    /// Find a template in the registry
    #[must_use]
    pub fn find_template(&self, attribute_name: &str) -> Option<Rc<VersionedAttribute>> {
        // Use the pre-sorted list to find the first (longest) matching template
        for (template_name, attribute) in &self.templates_by_length {
            if attribute_name.starts_with(template_name) {
                return Some(Rc::clone(attribute));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use crate::{
        advice::{DeprecatedAdvisor, EnumAdvisor, RegoAdvisor, StabilityAdvisor, TypeAdvisor},
        sample_attribute::SampleAttribute,
        sample_instrumentation_scope::SampleInstrumentationScope,
        sample_metric::{
            DataPoints, SampleExemplar, SampleExponentialHistogramDataPoint, SampleInstrument,
            SampleMetric, SampleNumberDataPoint,
        },
        sample_resource::SampleResource,
        sample_span::SampleSpan,
        LiveCheckRunner, LiveCheckStatistics, Sample,
    };

    use super::*;
    use crate::sample_log::SampleLog;
    use serde_json::json;
    use serde_yaml;
    use std::collections::BTreeMap;
    use weaver_checker::{FindingLevel, PolicyFinding};
    use weaver_forge::registry::{ResolvedGroup, ResolvedRegistry};
    use weaver_forge::v2::entity::{
        EntityAssociation as V2EntityAssociation, EntityAttribute, EntityRefinement,
    };
    use weaver_forge::v2::provenance::Provenance as V2Provenance;
    use weaver_forge::v2::{
        attribute::Attribute as V2Attribute,
        event::{Event as V2Event, EventAttribute},
        metric::{Metric as V2Metric, MetricAttribute},
        registry::{ForgeDependency, ForgeResolvedRegistry, Refinements, Registry},
        span::{Span as V2Span, SpanAttribute},
    };
    use weaver_resolved_schema::attribute::Attribute;
    use weaver_semconv::entity_association::EntityAssociation;
    use weaver_semconv::signal_requirement_level::SignalRequirementLevel;
    use weaver_semconv::v2::signal_id::SignalId;
    use weaver_semconv::v2::{span::SpanName, CommonFields};
    use weaver_semconv::{
        attribute::{
            AttributeType, BasicRequirementLevelSpec, EnumEntriesSpec, Examples,
            PrimitiveOrArrayTypeSpec, RequirementLevel, TemplateTypeSpec, ValueSpec,
        },
        group::{GroupType, InstrumentSpec, SpanKindSpec},
        stability::Stability,
        YamlValue,
    };
    fn get_all_advice(sample: &mut Sample) -> &mut [PolicyFinding] {
        match sample {
            Sample::Attribute(sample_attribute) => sample_attribute
                .live_check_result
                .as_mut() // Change to as_mut() to get a mutable reference
                .map(|result| &mut result.all_advice)
                .map_or(&mut [], |v| v),
            _ => &mut [],
        }
    }

    #[test]
    fn test_attribute_live_checker() {
        run_attribute_live_checker_test(false);
    }

    #[test]
    fn test_attribute_live_checker_v2() {
        run_attribute_live_checker_test(true);
    }

    fn run_attribute_live_checker_test(use_v2: bool) {
        let registry = make_registry(use_v2);

        let mut samples = vec![
            Sample::Attribute(SampleAttribute::try_from("test.string=value").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("testString2").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.deprecated=42").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("aws.s3.bucket.name").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.enum=foo").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.enum=example_variant1").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.enum=42.42").unwrap()),
            Sample::Attribute(
                SampleAttribute::try_from("test.string.not.allowed=example_value").unwrap(),
            ),
            Sample::Attribute(SampleAttribute::try_from("test.extends=new_value").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.template.my.key=42").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.deprecated.allowed=42").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.enum=17").unwrap()),
        ];

        let advisors: Vec<Box<dyn Advisor>> = vec![
            Box::new(DeprecatedAdvisor),
            Box::new(StabilityAdvisor),
            Box::new(TypeAdvisor),
            Box::new(EnumAdvisor),
        ];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor = RegoAdvisor::new(&live_checker, &None, &None, &None)
            .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize(live_checker.matchers());

        let all_advice = get_all_advice(&mut samples[0]);
        assert!(all_advice.is_empty());

        let all_advice = get_all_advice(&mut samples[1]);
        assert_eq!(all_advice.len(), 3);
        // make a sort of the advice
        all_advice.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(all_advice[0].id, "invalid_format");
        assert_eq!(
            all_advice[0].context,
            Some(json!({"attribute_key": "testString2" }))
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute key 'testString2' does not match name formatting rules."
        );
        assert_eq!(all_advice[1].id, "missing_attribute");
        assert_eq!(
            all_advice[1].context,
            Some(json!({"attribute_key": "testString2"}))
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute 'testString2' does not exist in the registry."
        );
        assert_eq!(all_advice[2].id, "missing_namespace");
        assert_eq!(
            all_advice[2].context,
            Some(json!({"attribute_key": "testString2"}))
        );
        assert_eq!(all_advice[2].message, "Attribute key 'testString2' must include a namespace (e.g. '{namespace}.{attribute_key}')");

        let all_advice = get_all_advice(&mut samples[2]);
        assert_eq!(all_advice.len(), 3);
        assert_eq!(all_advice[0].id, "deprecated");
        assert_eq!(
            all_advice[0].context,
            Some(
                json!({"attribute_key": "test.deprecated", "deprecation_reason": "uncategorized", "deprecation_note": "note"})
            )
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'test.deprecated' is deprecated; reason = 'uncategorized', note = 'note'."
        );

        assert_eq!(all_advice[1].id, "not_stable");
        assert_eq!(
            all_advice[1].context,
            Some(json!({"attribute_key": "test.deprecated", "stability": "development"}))
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute 'test.deprecated' is not stable; stability = development."
        );

        assert_eq!(all_advice[2].id, "type_mismatch");
        assert_eq!(
            all_advice[2].context,
            Some(
                json!({"attribute_key": "test.deprecated", "attribute_type": "int", "expected": "string"})
            )
        );
        assert_eq!(
            all_advice[2].message,
            "Attribute 'test.deprecated' has type 'int'. Type should be 'string'."
        );

        let all_advice = get_all_advice(&mut samples[3]);
        assert_eq!(all_advice.len(), 1);
        assert_eq!(all_advice[0].id, "missing_attribute");
        assert_eq!(
            all_advice[0].context,
            Some(json!({"attribute_key": "aws.s3.bucket.name"}))
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'aws.s3.bucket.name' does not exist in the registry."
        );

        let all_advice = get_all_advice(&mut samples[4]);
        assert_eq!(all_advice.len(), 1);
        assert_eq!(all_advice[0].id, "undefined_enum_variant");
        assert_eq!(
            all_advice[0].context,
            Some(json!({"attribute_key": "test.enum", "attribute_value": "foo"}))
        );
        assert_eq!(
            all_advice[0].message,
            "Enum attribute 'test.enum' has value 'foo' which is not documented."
        );

        let all_advice = get_all_advice(&mut samples[6]);
        assert_eq!(all_advice.len(), 1);
        assert_eq!(all_advice[0].id, "type_mismatch");
        assert_eq!(
            all_advice[0].context,
            Some(json!({"attribute_key": "test.enum", "attribute_type": "double"}))
        );
        assert_eq!(all_advice[0].message, "Enum attribute 'test.enum' has type 'double'. Enum value type should be 'string' or 'int'.");

        let all_advice = get_all_advice(&mut samples[7]);

        // Make a sort of the advice
        all_advice.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(all_advice.len(), 3);

        assert_eq!(all_advice[0].id, "extends_namespace");
        assert_eq!(
            all_advice[0].context,
            Some(json!({"attribute_key": "test.string.not.allowed", "namespace": "test"}))
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute key 'test.string.not.allowed' collides with existing namespace 'test'"
        );
        assert_eq!(all_advice[1].id, "illegal_namespace");
        assert_eq!(
            all_advice[1].context,
            Some(json!({"attribute_key": "test.string.not.allowed", "namespace": "test.string"}))
        );
        assert_eq!(
            all_advice[1].message,
            "Namespace 'test.string' collides with existing attribute 'test.string.not.allowed'"
        );
        assert_eq!(all_advice[2].id, "missing_attribute");
        assert_eq!(
            all_advice[2].context,
            Some(json!({
                "attribute_key": "test.string.not.allowed"
            }))
        );
        assert_eq!(
            all_advice[2].message,
            "Attribute 'test.string.not.allowed' does not exist in the registry."
        );

        let all_advice = get_all_advice(&mut samples[8]);
        assert_eq!(all_advice.len(), 2);
        assert_eq!(all_advice[0].id, "missing_attribute");
        assert_eq!(
            all_advice[0].context,
            Some(json!({"attribute_key": "test.extends"}))
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'test.extends' does not exist in the registry."
        );
        assert_eq!(all_advice[1].id, "extends_namespace");
        assert_eq!(
            all_advice[1].context,
            Some(json!({"attribute_key": "test.extends", "namespace": "test"}))
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute key 'test.extends' collides with existing namespace 'test'"
        );

        // test.template
        let all_advice = get_all_advice(&mut samples[9]);
        assert_eq!(all_advice.len(), 2);
        assert_eq!(all_advice[0].id, "template_attribute");
        assert_eq!(
            all_advice[0].context,
            Some(
                json!({"attribute_key": "test.template.my.key", "template_name": "test.template"})
            )
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'test.template.my.key' is a template"
        );
        assert_eq!(all_advice[1].id, "type_mismatch");
        assert_eq!(
            all_advice[1].context,
            Some(
                json!({"attribute_key": "test.template.my.key", "attribute_type": "int", "expected": "string"})
            )
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute 'test.template.my.key' has type 'int'. Type should be 'string'."
        );

        // test.deprecated.allowed
        // Should not get illegal_namespace for extending a deprecated attribute
        let all_advice = get_all_advice(&mut samples[10]);
        assert_eq!(all_advice.len(), 2);
        assert_eq!(all_advice[0].id, "missing_attribute");
        assert_eq!(
            all_advice[0].context,
            Some(json!({"attribute_key": "test.deprecated.allowed"}))
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'test.deprecated.allowed' does not exist in the registry."
        );
        assert_eq!(all_advice[1].id, "extends_namespace");
        assert_eq!(
            all_advice[1].context,
            Some(json!({"attribute_key": "test.deprecated.allowed", "namespace": "test"}))
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute key 'test.deprecated.allowed' collides with existing namespace 'test'"
        );

        let all_advice = get_all_advice(&mut samples[11]);
        assert_eq!(all_advice.len(), 1);
        assert_eq!(all_advice[0].id, "undefined_enum_variant");
        assert_eq!(
            all_advice[0].context,
            Some(json!({"attribute_key": "test.enum", "attribute_value": 17}))
        );
        assert_eq!(
            all_advice[0].message,
            "Enum attribute 'test.enum' has value '17' which is not documented."
        );

        // Check statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(cumulative_stats.total_entities, 12);
            assert_eq!(cumulative_stats.total_advisories, 19);
            assert_eq!(cumulative_stats.advice_level_counts.len(), 3);
            assert_eq!(
                cumulative_stats.advice_level_counts[&FindingLevel::Violation],
                11
            );
            assert_eq!(
                cumulative_stats.advice_level_counts[&FindingLevel::Information],
                6
            );
            assert_eq!(
                cumulative_stats.advice_level_counts[&FindingLevel::Improvement],
                2
            );
            assert_eq!(cumulative_stats.highest_advice_level_counts.len(), 2);
            assert_eq!(
                cumulative_stats.highest_advice_level_counts[&FindingLevel::Violation],
                8
            );
            assert_eq!(
                cumulative_stats.highest_advice_level_counts[&FindingLevel::Information],
                2
            );
            assert_eq!(cumulative_stats.no_advice_count, 2);
            assert_eq!(cumulative_stats.seen_registry_attributes.len(), 3);
            assert_eq!(cumulative_stats.seen_registry_attributes["test.enum"], 4);
            assert_eq!(cumulative_stats.seen_non_registry_attributes.len(), 6);
            assert_eq!(cumulative_stats.registry_coverage, 1.0);
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    fn make_registry(use_v2: bool) -> VersionedRegistry {
        if use_v2 {
            VersionedRegistry::V2(Box::new(ForgeResolvedRegistry {
                schema_url: "https://example.com/schemas/1.2.3"
                    .try_into()
                    .expect("Should be valid schema url"),
                registry: Registry {
                    attributes: vec![
                        V2Attribute {
                            key: "test.string".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: Some(Examples::Strings(vec![
                                "value1".to_owned(),
                                "value2".to_owned(),
                            ])),
                            common: CommonFields {
                                brief: "".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Stable,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                            provenance: Default::default(),
                        },
                        V2Attribute {
                            key: "test.enum".to_owned(),
                            r#type: AttributeType::Enum {
                                members: vec![
                                    EnumEntriesSpec {
                                        id: "test_enum_member".to_owned(),
                                        value: ValueSpec::String("example_variant1".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Stable),
                                        deprecated: None,
                                        annotations: None,
                                    },
                                    EnumEntriesSpec {
                                        id: "test_enum_member2".to_owned(),
                                        value: ValueSpec::String("example_variant2".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Stable),
                                        deprecated: None,
                                        annotations: None,
                                    },
                                ],
                            },
                            examples: None,
                            common: CommonFields {
                                brief: "".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Stable,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                            provenance: Default::default(),
                        },
                        V2Attribute {
                            key: "test.deprecated".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: Some(Examples::Strings(vec![
                                "value1".to_owned(),
                                "value2".to_owned(),
                            ])),
                            common: CommonFields {
                                brief: "".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Development,
                                deprecated: Some(
                                    weaver_semconv::deprecated::Deprecated::Uncategorized {
                                        note: "note".to_owned(),
                                    },
                                ),
                                annotations: BTreeMap::new(),
                            },
                            provenance: Default::default(),
                        },
                        V2Attribute {
                            key: "test.template".to_owned(),
                            r#type: AttributeType::Template(TemplateTypeSpec::String),
                            examples: Some(Examples::Strings(vec![
                                "value1".to_owned(),
                                "value2".to_owned(),
                            ])),
                            common: CommonFields {
                                brief: "".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Stable,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                            provenance: Default::default(),
                        },
                    ],
                    attribute_groups: vec![],
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                    entities: vec![],
                },
                refinements: Refinements {
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                    entities: vec![],
                },
                dependencies: Default::default(),
                dependency_graph: Default::default(),
            }))
        } else {
            VersionedRegistry::V1(Box::new(ResolvedRegistry {
                registry_url: "TEST".to_owned(),
                groups: vec![ResolvedGroup {
                    id: "test.comprehensive.internal".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    entity_associations: vec![],
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![
                        Attribute {
                            name: "test.string".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: Some(Examples::Strings(vec![
                                "value1".to_owned(),
                                "value2".to_owned(),
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                            annotations: None,
                            role: Default::default(),
                        },
                        Attribute {
                            name: "test.enum".to_owned(),
                            r#type: AttributeType::Enum {
                                members: vec![
                                    EnumEntriesSpec {
                                        id: "test_enum_member".to_owned(),
                                        value: ValueSpec::String("example_variant1".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Stable),
                                        deprecated: None,
                                        annotations: None,
                                    },
                                    EnumEntriesSpec {
                                        id: "test_enum_member2".to_owned(),
                                        value: ValueSpec::String("example_variant2".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Stable),
                                        deprecated: None,
                                        annotations: None,
                                    },
                                ],
                            },
                            examples: None,
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                            annotations: None,
                            role: Default::default(),
                        },
                        Attribute {
                            name: "test.deprecated".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: Some(Examples::Strings(vec![
                                "value1".to_owned(),
                                "value2".to_owned(),
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Development),
                            deprecated: Some(
                                weaver_semconv::deprecated::Deprecated::Uncategorized {
                                    note: "note".to_owned(),
                                },
                            ),
                            prefix: false,
                            tags: None,
                            value: None,
                            annotations: None,
                            role: Default::default(),
                        },
                        Attribute {
                            name: "test.template".to_owned(),
                            r#type: AttributeType::Template(TemplateTypeSpec::String),
                            examples: Some(Examples::Strings(vec![
                                "value1".to_owned(),
                                "value2".to_owned(),
                            ])),
                            brief: "".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                            annotations: None,
                            role: Default::default(),
                        },
                    ],
                    span_kind: Some(SpanKindSpec::Internal),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    requirement_level: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                }],
            }))
        }
    }

    fn make_metrics_registry(use_v2: bool) -> VersionedRegistry {
        if use_v2 {
            let memory_state_attr = V2Attribute {
                key: "system.memory.state".to_owned(),
                r#type: AttributeType::Enum {
                    members: vec![
                        EnumEntriesSpec {
                            id: "used".to_owned(),
                            value: ValueSpec::String("used".to_owned()),
                            brief: None,
                            note: None,
                            stability: Some(Stability::Development),
                            deprecated: None,
                            annotations: None,
                        },
                        EnumEntriesSpec {
                            id: "free".to_owned(),
                            value: ValueSpec::String("free".to_owned()),
                            brief: None,
                            note: None,
                            stability: Some(Stability::Development),
                            deprecated: None,
                            annotations: None,
                        },
                    ],
                },
                examples: Some(Examples::Strings(vec![
                    "free".to_owned(),
                    "cached".to_owned(),
                ])),
                common: CommonFields {
                    brief: "The memory state".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Development,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            };

            VersionedRegistry::V2(Box::new(ForgeResolvedRegistry {
                schema_url: "https://example.com/schemas/1.2.3"
                    .try_into()
                    .expect("Should be valid schema url"),
                registry: Registry {
                    attributes: vec![memory_state_attr.clone()],
                    attribute_groups: vec![],
                    metrics: vec![
                        V2Metric {
                            name: "system.uptime".to_owned().into(),
                            instrument: InstrumentSpec::Gauge,
                            unit: "s".to_owned(),
                            requirement_level: Some(SignalRequirementLevel::OptIn),
                            attributes: vec![],
                            entity_associations: vec![],
                            common: CommonFields {
                                brief: "The time the system has been running".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Development,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                            provenance: Default::default(),
                        },
                        V2Metric {
                            name: "system.memory.usage".to_owned().into(),
                            instrument: InstrumentSpec::UpDownCounter,
                            unit: "By".to_owned(),
                            requirement_level: Some(SignalRequirementLevel::OptIn),
                            attributes: vec![MetricAttribute {
                                base: memory_state_attr.clone(),
                                requirement_level: RequirementLevel::Recommended {
                                    text: "".to_owned(),
                                },
                            }],
                            entity_associations: vec![],
                            common: CommonFields {
                                brief: "Reports memory in use by state.".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Development,
                                deprecated: None,
                                annotations: BTreeMap::new(),
                            },
                            provenance: Default::default(),
                        },
                    ],
                    spans: vec![],
                    events: vec![],
                    entities: vec![],
                },
                refinements: Refinements {
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                    entities: vec![],
                },
                dependencies: Default::default(),
                dependency_graph: Default::default(),
            }))
        } else {
            VersionedRegistry::V1(Box::new(ResolvedRegistry {
                registry_url: "TEST_METRICS".to_owned(),
                groups: vec![
                    // Attribute group for system memory
                    ResolvedGroup {
                        id: "registry.system.memory".to_owned(),
                        r#type: GroupType::AttributeGroup,
                        brief: "Describes System Memory attributes".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: None,
                        deprecated: None,
                        attributes: vec![Attribute {
                            name: "system.memory.state".to_owned(),
                            r#type: AttributeType::Enum {
                                members: vec![
                                    EnumEntriesSpec {
                                        id: "used".to_owned(),
                                        value: ValueSpec::String("used".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Development),
                                        deprecated: None,
                                        annotations: None,
                                    },
                                    EnumEntriesSpec {
                                        id: "free".to_owned(),
                                        value: ValueSpec::String("free".to_owned()),
                                        brief: None,
                                        note: None,
                                        stability: Some(Stability::Development),
                                        deprecated: None,
                                        annotations: None,
                                    },
                                ],
                            },
                            examples: Some(Examples::Strings(vec![
                                "free".to_owned(),
                                "cached".to_owned(),
                            ])),
                            brief: "The memory state".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Development),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                            annotations: None,
                            role: Default::default(),
                        }],
                        span_kind: None,
                        events: vec![],
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        requirement_level: None,
                        name: None,
                        lineage: None,
                        display_name: Some("System Memory Attributes".to_owned()),
                        body: None,
                        annotations: None,
                    },
                    // System uptime metric
                    ResolvedGroup {
                        id: "metric.system.uptime".to_owned(),
                        r#type: GroupType::Metric,
                        brief: "The time the system has been running".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: Some(Stability::Development),
                        deprecated: None,
                        attributes: vec![],
                        span_kind: None,
                        events: vec![],
                        metric_name: Some("system.uptime".to_owned()),
                        instrument: Some(InstrumentSpec::Gauge),
                        unit: Some("s".to_owned()),
                        requirement_level: Some(SignalRequirementLevel::Recommended),
                        name: None,
                        lineage: None,
                        display_name: None,
                        body: None,
                        annotations: None,
                    },
                    // System memory usage metric
                    ResolvedGroup {
                        id: "metric.system.memory.usage".to_owned(),
                        r#type: GroupType::Metric,
                        brief: "Reports memory in use by state.".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: Some(Stability::Development),
                        deprecated: None,
                        attributes: vec![Attribute {
                            name: "system.memory.state".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: None,
                            brief: "The memory state".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Development),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                            annotations: None,
                            role: Default::default(),
                        }],
                        span_kind: None,
                        events: vec![],
                        metric_name: Some("system.memory.usage".to_owned()),
                        instrument: Some(InstrumentSpec::UpDownCounter),
                        unit: Some("By".to_owned()),
                        requirement_level: Some(SignalRequirementLevel::Recommended),
                        name: None,
                        lineage: None,
                        display_name: None,
                        body: None,
                        annotations: None,
                    },
                ],
            }))
        }
    }

    fn make_custom_rego_registry(use_v2: bool) -> VersionedRegistry {
        if use_v2 {
            let custom_string_attr = V2Attribute {
                key: "custom.string".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: Some(Examples::Strings(vec![
                    "value1".to_owned(),
                    "value2".to_owned(),
                ])),
                common: CommonFields {
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            };

            VersionedRegistry::V2(Box::new(ForgeResolvedRegistry {
                schema_url: "https://example.com/schemas/1.2.3"
                    .try_into()
                    .expect("Should be valid schema url"),
                registry: Registry {
                    attributes: vec![custom_string_attr.clone()],
                    attribute_groups: vec![],
                    metrics: vec![],
                    spans: vec![V2Span {
                        requirement_level: None,
                        r#type: "custom.comprehensive.internal".to_owned().into(),
                        kind: SpanKindSpec::Internal,
                        name: SpanName {
                            note: "custom.comprehensive.internal".to_owned(),
                        },
                        attributes: vec![SpanAttribute {
                            base: custom_string_attr.clone(),
                            requirement_level: RequirementLevel::Recommended {
                                text: "".to_owned(),
                            },
                            sampling_relevant: None,
                        }],
                        entity_associations: vec![],
                        common: CommonFields {
                            brief: "".to_owned(),
                            note: "".to_owned(),
                            stability: Stability::Stable,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    }],
                    events: vec![],
                    entities: vec![],
                },
                refinements: Refinements {
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                    entities: vec![],
                },
                dependencies: Default::default(),
                dependency_graph: Default::default(),
            }))
        } else {
            VersionedRegistry::V1(Box::new(ResolvedRegistry {
                registry_url: "TEST".to_owned(),
                groups: vec![ResolvedGroup {
                    id: "custom.comprehensive.internal".to_owned(),
                    r#type: GroupType::Span,
                    brief: "".to_owned(),
                    note: "".to_owned(),
                    prefix: "".to_owned(),
                    entity_associations: vec![],
                    extends: None,
                    stability: Some(Stability::Stable),
                    deprecated: None,
                    attributes: vec![Attribute {
                        name: "custom.string".to_owned(),
                        r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                        examples: Some(Examples::Strings(vec![
                            "value1".to_owned(),
                            "value2".to_owned(),
                        ])),
                        brief: "".to_owned(),
                        tag: None,
                        requirement_level: RequirementLevel::Recommended {
                            text: "".to_owned(),
                        },
                        sampling_relevant: None,
                        note: "".to_owned(),
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        prefix: false,
                        tags: None,
                        value: None,
                        annotations: None,
                        role: Default::default(),
                    }],
                    span_kind: Some(SpanKindSpec::Internal),
                    events: vec![],
                    metric_name: None,
                    instrument: None,
                    unit: None,
                    requirement_level: None,
                    name: None,
                    lineage: None,
                    display_name: None,
                    body: None,
                    annotations: None,
                }],
            }))
        }
    }

    #[test]
    fn test_custom_rego() {
        run_custom_rego_test(false);
    }

    #[test]
    fn test_custom_rego_v2() {
        run_custom_rego_test(true);
    }

    fn run_custom_rego_test(use_v2: bool) {
        let registry = make_custom_rego_registry(use_v2);

        let mut samples = vec![
            Sample::Attribute(SampleAttribute::try_from("custom.string=hello").unwrap()),
            Sample::Attribute(SampleAttribute::try_from("test.string").unwrap()),
        ];

        let advisors: Vec<Box<dyn Advisor>> = vec![];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/live_check_advice/".into()),
            &Some("data/jq/test.jq".into()),
            &None,
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize(live_checker.matchers());

        let all_advice = get_all_advice(&mut samples[0]);
        assert!(all_advice.is_empty());

        let all_advice = get_all_advice(&mut samples[1]);
        assert_eq!(all_advice.len(), 2);

        assert_eq!(all_advice[0].id, "missing_attribute");
        assert_eq!(
            all_advice[0].context,
            Some(json!({"attribute_key": "test.string"}))
        );
        assert_eq!(
            all_advice[0].message,
            "Attribute 'test.string' does not exist in the registry."
        );
        assert_eq!(all_advice[1].id, "contains_test");
        assert_eq!(
            all_advice[1].context,
            Some(json!({"attribute_key": "test.string"}))
        );
        assert_eq!(
            all_advice[1].message,
            "Attribute name must not contain 'test', but was 'test.string'"
        );

        // Check statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(cumulative_stats.total_entities, 2);
            assert_eq!(cumulative_stats.total_advisories, 2);
            assert_eq!(cumulative_stats.advice_level_counts.len(), 1);
            assert_eq!(
                cumulative_stats.advice_level_counts[&FindingLevel::Violation],
                2
            );
            assert_eq!(cumulative_stats.highest_advice_level_counts.len(), 1);
            assert_eq!(
                cumulative_stats.highest_advice_level_counts[&FindingLevel::Violation],
                1
            );
            assert_eq!(cumulative_stats.no_advice_count, 1);
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_json_input_output() {
        run_json_input_output_test(false);
    }

    #[test]
    fn test_json_input_output_v2() {
        run_json_input_output_test(true);
    }

    fn run_json_input_output_test(use_v2: bool) {
        let registry = make_registry(use_v2);

        // Load samples from JSON file
        let path = "data/span.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let advisors: Vec<Box<dyn Advisor>> = vec![
            Box::new(DeprecatedAdvisor),
            Box::new(StabilityAdvisor),
            Box::new(TypeAdvisor),
            Box::new(EnumAdvisor),
        ];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor = RegoAdvisor::new(&live_checker, &None, &None, &None)
            .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize(live_checker.matchers());

        // Check the statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(cumulative_stats.total_entities, 14);
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("attribute"),
                Some(&10)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("span"),
                Some(&1)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("span_event"),
                Some(&1)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("span_link"),
                Some(&1)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("resource"),
                Some(&1)
            );
            assert_eq!(cumulative_stats.total_advisories, 14);
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_json_span_rego() {
        run_json_span_rego_test(false);
    }

    #[test]
    fn test_json_span_rego_v2() {
        run_json_span_rego_test(true);
    }

    fn run_json_span_rego_test(use_v2: bool) {
        let registry = make_registry(use_v2);

        // Load samples from JSON file
        let path = "data/span.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let mut live_checker = LiveChecker::new(Arc::new(registry), vec![]);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/live_check_advice/".into()),
            &Some("data/jq/test.jq".into()),
            &None,
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize(live_checker.matchers());

        // Check the statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("contains_test_in_status"),
                Some(&1)
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_json_metric() {
        run_json_metric_test(false);
    }

    #[test]
    fn test_json_metric_v2() {
        run_json_metric_test(true);
    }

    fn run_json_metric_test(use_v2: bool) {
        let registry = make_metrics_registry(use_v2);

        // Load samples from JSON file
        let path = "data/metrics.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let advisors: Vec<Box<dyn Advisor>> = vec![
            Box::new(DeprecatedAdvisor),
            Box::new(StabilityAdvisor),
            Box::new(TypeAdvisor),
            Box::new(EnumAdvisor),
        ];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor = RegoAdvisor::new(&live_checker, &None, &None, &None)
            .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize(live_checker.matchers());

        // Check the statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("data_point"),
                Some(&6)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("metric"),
                Some(&4)
            );
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("attribute"),
                Some(&3)
            );
            assert_eq!(cumulative_stats.no_advice_count, 4);
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("recommended_attribute_not_present"),
                Some(&2)
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("missing_attribute"),
                Some(&2)
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("not_stable"),
                Some(&2)
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("missing_metric"),
                Some(&3)
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("missing_namespace"),
                Some(&2)
            );
            assert_eq!(
                cumulative_stats
                    .seen_registry_metrics
                    .get("system.memory.usage"),
                Some(&1)
            );
            assert_eq!(cumulative_stats.seen_non_registry_metrics.len(), 3);
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_json_metric_custom_rego() {
        run_json_metric_custom_rego_test(false);
    }

    #[test]
    fn test_json_metric_custom_rego_v2() {
        run_json_metric_custom_rego_test(true);
    }

    fn run_json_metric_custom_rego_test(use_v2: bool) {
        let registry = make_metrics_registry(use_v2);

        // Load samples from JSON file
        let path = "data/metrics.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let mut live_checker = LiveChecker::new(Arc::new(registry), vec![]);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/live_check_advice/".into()),
            &Some("data/jq/test.jq".into()),
            &None,
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());

            assert!(result.is_ok());
        }
        stats.finalize(live_checker.matchers());
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("invalid_data_point_value"),
                Some(&1)
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_json_log_custom_rego() {
        run_json_log_custom_rego_test(false);
    }

    #[test]
    fn test_json_log_custom_rego_v2() {
        run_json_log_custom_rego_test(true);
    }

    fn run_json_log_custom_rego_test(use_v2: bool) {
        let registry = make_events_registry(use_v2);

        // Load samples from JSON file
        let path = "data/logs.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let mut live_checker = LiveChecker::new(Arc::new(registry), vec![]);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/live_check_advice/".into()),
            &Some("data/jq/test.jq".into()),
            &None,
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());

            assert!(result.is_ok());
        }
        stats.finalize(live_checker.matchers());

        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats.advice_type_counts.get("empty_body"),
                Some(&1),
                "Expected 1 empty_body advice for event with empty name and empty body"
            );

            assert_eq!(
                cumulative_stats.advice_type_counts.get("required_phrase_missing"),
                Some(&1),
                "Expected 1 required_phrase_missing advice for session.start event missing 'hello world'"
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    fn make_events_registry(use_v2: bool) -> VersionedRegistry {
        if use_v2 {
            let session_id_attr = V2Attribute {
                key: "session.id".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: Some(Examples::Strings(vec![
                    "00112233-4455-6677-8899-aabbccddeeff".to_owned(),
                ])),
                common: CommonFields {
                    brief: "A unique session identifier".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Development,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            };

            let session_previous_id_attr = V2Attribute {
                key: "session.previous_id".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: Some(Examples::Strings(vec![
                    "00112233-4455-6677-8899-aabbccddeeff".to_owned(),
                ])),
                common: CommonFields {
                    brief: "The previous session identifier".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Development,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            };

            VersionedRegistry::V2(Box::new(ForgeResolvedRegistry {
                schema_url: "https://example.com/schemas/1.2.3"
                    .try_into()
                    .expect("Should be valid schema url"),
                registry: Registry {
                    attributes: vec![session_id_attr.clone(), session_previous_id_attr.clone()],
                    attribute_groups: vec![],
                    metrics: vec![],
                    spans: vec![],
                    events: vec![
                        V2Event {
                            requirement_level: None,
                            name: "session.start".to_owned().into(),
                            attributes: vec![
                                EventAttribute {
                                    base: session_id_attr.clone(),
                                    requirement_level: RequirementLevel::Basic(
                                        BasicRequirementLevelSpec::Required,
                                    ),
                                },
                                EventAttribute {
                                    base: session_previous_id_attr.clone(),
                                    requirement_level: RequirementLevel::Recommended {
                                        text: "".to_owned(),
                                    },
                                },
                            ],
                            entity_associations: vec![],
                            common: CommonFields {
                                brief: "This event represents a session start".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Development,
                                deprecated: Some(
                                    weaver_semconv::deprecated::Deprecated::Uncategorized {
                                        note: "Use session.initialized event instead".to_owned(),
                                    },
                                ),
                                annotations: {
                                    let mut annotations = BTreeMap::new();
                                    let _ = annotations.insert(
                                        "required_phrase".to_owned(),
                                        YamlValue(serde_yaml::Value::String(
                                            "hello world".to_owned(),
                                        )),
                                    );
                                    annotations
                                },
                            },
                            provenance: Default::default(),
                        },
                        V2Event {
                            requirement_level: None,
                            name: "example.event".to_owned().into(),
                            attributes: vec![],
                            entity_associations: vec![],
                            common: CommonFields {
                                brief: "An example event".to_owned(),
                                note: "".to_owned(),
                                stability: Stability::Stable,
                                deprecated: None,
                                annotations: {
                                    let mut annotations = BTreeMap::new();
                                    let _ = annotations.insert(
                                        "required_phrase".to_owned(),
                                        YamlValue(serde_yaml::Value::String(
                                            "hello world".to_owned(),
                                        )),
                                    );
                                    annotations
                                },
                            },
                            provenance: Default::default(),
                        },
                    ],
                    entities: vec![],
                },
                refinements: Refinements {
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                    entities: vec![],
                },
                dependencies: Default::default(),
                dependency_graph: Default::default(),
            }))
        } else {
            VersionedRegistry::V1(Box::new(ResolvedRegistry {
                registry_url: "TEST_EVENTS".to_owned(),
                groups: vec![
                    ResolvedGroup {
                        id: "event.session.start".to_owned(),
                        r#type: GroupType::Event,
                        brief: "This event represents a session start".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: Some(Stability::Development),
                        deprecated: Some(weaver_semconv::deprecated::Deprecated::Uncategorized {
                            note: "Use session.initialized event instead".to_owned(),
                        }),
                        attributes: vec![
                            Attribute {
                                name: "session.id".to_owned(),
                                r#type: AttributeType::PrimitiveOrArray(
                                    PrimitiveOrArrayTypeSpec::String,
                                ),
                                examples: Some(Examples::Strings(vec![
                                    "00112233-4455-6677-8899-aabbccddeeff".to_owned(),
                                ])),
                                brief: "A unique session identifier".to_owned(),
                                tag: None,
                                requirement_level: RequirementLevel::Basic(
                                    BasicRequirementLevelSpec::Required,
                                ),
                                sampling_relevant: None,
                                note: "".to_owned(),
                                stability: Some(Stability::Development),
                                deprecated: None,
                                prefix: false,
                                tags: None,
                                value: None,
                                annotations: None,
                                role: Default::default(),
                            },
                            Attribute {
                                name: "session.previous_id".to_owned(),
                                r#type: AttributeType::PrimitiveOrArray(
                                    PrimitiveOrArrayTypeSpec::String,
                                ),
                                examples: Some(Examples::Strings(vec![
                                    "00112233-4455-6677-8899-aabbccddeeff".to_owned(),
                                ])),
                                brief: "The previous session identifier".to_owned(),
                                tag: None,
                                requirement_level: RequirementLevel::Recommended {
                                    text: "".to_owned(),
                                },
                                sampling_relevant: None,
                                note: "".to_owned(),
                                stability: Some(Stability::Development),
                                deprecated: None,
                                prefix: false,
                                tags: None,
                                value: None,
                                annotations: None,
                                role: Default::default(),
                            },
                        ],
                        span_kind: None,
                        events: vec![],
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        requirement_level: None,
                        name: Some("session.start".to_owned()),
                        lineage: None,
                        display_name: Some("Session Start Event".to_owned()),
                        body: None,
                        annotations: Some({
                            let mut annotations = BTreeMap::new();
                            let _ = annotations.insert(
                                "required_phrase".to_owned(),
                                YamlValue(serde_yaml::Value::String("hello world".to_owned())),
                            );
                            annotations
                        }),
                    },
                    ResolvedGroup {
                        id: "event.example.event".to_owned(),
                        r#type: GroupType::Event,
                        brief: "An example event".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        attributes: vec![],
                        span_kind: None,
                        events: vec![],
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        requirement_level: None,
                        name: Some("example.event".to_owned()),
                        lineage: None,
                        display_name: Some("Example Event".to_owned()),
                        body: None,
                        annotations: Some({
                            let mut annotations = BTreeMap::new();
                            let _ = annotations.insert(
                                "required_phrase".to_owned(),
                                YamlValue(serde_yaml::Value::String("hello world".to_owned())),
                            );
                            annotations
                        }),
                    },
                ],
            }))
        }
    }

    #[test]
    fn test_json_log() {
        run_json_log_test(false);
    }

    #[test]
    fn test_json_log_v2() {
        run_json_log_test(true);
    }

    fn run_json_log_test(use_v2: bool) {
        let registry = make_events_registry(use_v2);

        // Load samples from JSON file
        let path = "data/logs.json";
        let mut samples: Vec<Sample> =
            serde_json::from_reader(File::open(path).expect("Unable to open file"))
                .expect("Unable to parse JSON");

        let advisors: Vec<Box<dyn Advisor>> = vec![
            Box::new(DeprecatedAdvisor),
            Box::new(StabilityAdvisor),
            Box::new(TypeAdvisor),
            Box::new(EnumAdvisor),
        ];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor = RegoAdvisor::new(&live_checker, &None, &None, &None)
            .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize(live_checker.matchers());

        // Check the statistics
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(cumulative_stats.total_entities, 8);
            assert_eq!(cumulative_stats.total_entities_by_type.get("log"), Some(&4));
            assert_eq!(
                cumulative_stats.total_entities_by_type.get("attribute"),
                Some(&4)
            );

            // Check advisor advice types
            // Expected advice:
            // - missing_attribute: 1 (session.idd does not exist in registry)
            // - required_attribute_not_present: 1 (session.id is required but not present)
            // - not_stable: 2 (1 for session.start event, 1 for session.previous_id attribute)
            // - deprecated: 1 (session.start event is deprecated)
            // - missing_event: 1 (session.test event does not exist in registry)
            assert_eq!(
                cumulative_stats.advice_type_counts.get("missing_attribute"),
                Some(&1),
                "Expected 1 missing_attribute advice for session.idd"
            );
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("required_attribute_not_present"),
                Some(&1),
                "Expected 1 required_attribute_not_present advice for session.id"
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("not_stable"),
                Some(&4),
                "Expected 4 not_stable advice (for session.start event, session.previous_id attribute, and 2 session.id attribute)"
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("deprecated"),
                Some(&1),
                "Expected 1 deprecated advice for session.start event"
            );
            assert_eq!(
                cumulative_stats.advice_type_counts.get("missing_event"),
                Some(&1),
                "Expected 1 missing_event advice for session.test"
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_bad_custom_rego() {
        run_bad_custom_rego_test(false);
    }

    #[test]
    fn test_bad_custom_rego_v2() {
        run_bad_custom_rego_test(true);
    }

    fn run_bad_custom_rego_test(use_v2: bool) {
        let registry = make_custom_rego_registry(use_v2);

        let mut samples = vec![Sample::Attribute(
            SampleAttribute::try_from("custom.string=hello").unwrap(),
        )];

        let advisors: Vec<Box<dyn Advisor>> = vec![];

        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/bad_advice/".into()),
            &Some("data/jq/test.jq".into()),
            &None,
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            // This should fail with: "error: use of undefined variable `attribu1te_name` is unsafe"

            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("use of undefined variable"));
        }
    }

    #[test]
    fn test_exponential_histogram() {
        run_exponential_histogram_test(false);
    }

    #[test]
    fn test_exponential_histogram_v2() {
        run_exponential_histogram_test(true);
    }

    fn run_exponential_histogram_test(use_v2: bool) {
        let registry = make_metrics_registry(use_v2);

        // A sample with exponential histogram data points
        let sample = Sample::Metric(SampleMetric {
            name: "system.memory.usage".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Histogram),
            unit: "By".to_owned(),
            data_points: Some(DataPoints::ExponentialHistogram(vec![
                SampleExponentialHistogramDataPoint {
                    attributes: vec![],
                    count: 0,
                    sum: None,
                    min: None,
                    max: None,
                    live_check_result: None,
                    scale: 1,
                    zero_count: 0,
                    positive: None,
                    negative: None,
                    flags: 0,
                    zero_threshold: 0.0,
                    exemplars: vec![],
                },
            ])),
            instrumentation_scope: None,
            live_check_result: None,
            resource: None,
        });
        let mut samples = vec![sample];
        let advisors: Vec<Box<dyn Advisor>> = vec![Box::new(TypeAdvisor)];
        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize(live_checker.matchers());
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("unexpected_instrument"),
                Some(&1)
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
        // Check the live check result for the sample has the correct instrument mismatch message
        let sample = match &samples[0] {
            Sample::Metric(m) => m,
            _ => panic!("Expected a Metric sample"),
        };
        let live_check_result = sample.live_check_result.as_ref().unwrap();
        // Get the instrument_mismatch from all_advice
        let advice = live_check_result
            .all_advice
            .iter()
            .find(|a| a.id == "unexpected_instrument")
            .expect("Expected unexpected_instrument advice");
        assert_eq!(
            advice.message,
            "Instrument should be 'updowncounter', but found 'histogram'."
        );
        assert_eq!(advice.signal_name, Some("system.memory.usage".to_owned()));
        assert_eq!(advice.signal_type, Some("metric".to_owned()));
    }

    #[test]
    fn test_gauge_exemplar_rego() {
        run_gauge_exemplar_rego_test(false);
    }

    #[test]
    fn test_gauge_exemplar_rego_v2() {
        run_gauge_exemplar_rego_test(true);
    }

    fn run_gauge_exemplar_rego_test(use_v2: bool) {
        let registry = make_metrics_registry(use_v2);

        // A gauge sample with an exemplar
        let mut sample = Sample::Metric(SampleMetric {
            name: "system.uptime".to_owned(),
            instrument: SampleInstrument::Supported(InstrumentSpec::Gauge),
            unit: "s".to_owned(),
            data_points: Some(DataPoints::Number(vec![SampleNumberDataPoint {
                attributes: vec![],
                value: json!(0.0),
                flags: 0,
                live_check_result: None,
                exemplars: vec![SampleExemplar {
                    timestamp: "".to_owned(),
                    value: json!(0.0),
                    filtered_attributes: vec![],
                    span_id: "".to_owned(),
                    trace_id: "".to_owned(),
                    live_check_result: None,
                }],
            }])),
            instrumentation_scope: None,
            live_check_result: None,
            resource: None,
        });
        let advisors: Vec<Box<dyn Advisor>> = vec![Box::new(TypeAdvisor)];
        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);

        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/live_check_advice/".into()),
            &Some("data/jq/test.jq".into()),
            &None,
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        let result = sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());

        assert!(result.is_ok());
        stats.finalize(live_checker.matchers());
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats.advice_type_counts.get("low_value"),
                Some(&1)
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_summary_unspecified() {
        run_summary_unspecified_test(false);
    }

    #[test]
    fn test_summary_unspecified_v2() {
        run_summary_unspecified_test(true);
    }

    fn run_summary_unspecified_test(use_v2: bool) {
        let registry = make_metrics_registry(use_v2);

        let mut samples = vec![
            Sample::Metric(SampleMetric {
                name: "system.memory.usage".to_owned(),
                instrument: SampleInstrument::Unsupported("Summary".to_owned()),
                unit: "By".to_owned(),
                data_points: None,
                instrumentation_scope: None,
                live_check_result: None,
                resource: None,
            }),
            Sample::Metric(SampleMetric {
                name: "system.memory.usage".to_owned(),
                instrument: SampleInstrument::Unsupported("Unspecified".to_owned()),
                unit: "By".to_owned(),
                data_points: None,
                instrumentation_scope: None,
                live_check_result: None,
                resource: None,
            }),
        ];
        let advisors: Vec<Box<dyn Advisor>> = vec![Box::new(TypeAdvisor)];
        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);

        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        for sample in &mut samples {
            let result =
                sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
            assert!(result.is_ok());
        }
        stats.finalize(live_checker.matchers());
        if let LiveCheckStatistics::Cumulative(cumulative_stats) = &stats {
            assert_eq!(
                cumulative_stats
                    .advice_type_counts
                    .get("unexpected_instrument"),
                Some(&2)
            );
        } else {
            panic!("Expected Cumulative statistics");
        }
    }

    #[test]
    fn test_entity_validation() {
        run_entity_validation_test(false);
    }

    #[test]
    fn test_entity_validation_v2() {
        run_entity_validation_test(true);
    }

    fn make_entity_registry(use_v2: bool) -> VersionedRegistry {
        // A "deployment" entity with:
        //   identity:    deployment.name  (Required)
        //   description: deployment.environment (Recommended)
        //                deployment.tier         (OptIn)
        //                deployment.region       (ConditionallyRequired)
        if use_v2 {
            use weaver_forge::v2::entity::{Entity as V2Entity, EntityAttribute};
            use weaver_semconv::v2::signal_id::SignalId;

            let deployment_name_attr = V2Attribute {
                key: "deployment.name".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: CommonFields {
                    brief: "The deployment name".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            };
            let deployment_env_attr = V2Attribute {
                key: "deployment.environment".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: CommonFields {
                    brief: "The deployment environment".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            };
            let deployment_tier_attr = V2Attribute {
                key: "deployment.tier".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: CommonFields {
                    brief: "The deployment tier".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            };
            let deployment_region_attr = V2Attribute {
                key: "deployment.region".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: CommonFields {
                    brief: "The deployment region".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            };

            VersionedRegistry::V2(Box::new(ForgeResolvedRegistry {
                schema_url: "https://example.com/schemas/1.0.0"
                    .try_into()
                    .expect("valid schema url"),
                registry: Registry {
                    attributes: vec![
                        deployment_name_attr.clone(),
                        deployment_env_attr.clone(),
                        deployment_tier_attr.clone(),
                        deployment_region_attr.clone(),
                    ],
                    attribute_groups: vec![],
                    metrics: vec![],
                    spans: vec![],
                    events: vec![V2Event {
                        requirement_level: None,
                        name: "deployment.started".to_owned().into(),
                        attributes: vec![],
                        entity_associations: vec![V2EntityAssociation::Ref(EntityRef::local(
                            "deployment".to_owned().into(),
                        ))],
                        common: CommonFields {
                            brief: "A deployment has started".to_owned(),
                            note: "".to_owned(),
                            stability: Stability::Stable,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    }],
                    entities: vec![V2Entity {
                        requirement_level: None,
                        r#type: SignalId::from("deployment".to_owned()),
                        identity: vec![EntityAttribute {
                            base: deployment_name_attr,
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                        }],
                        description: vec![
                            EntityAttribute {
                                base: deployment_env_attr,
                                requirement_level: RequirementLevel::Recommended {
                                    text: "".to_owned(),
                                },
                            },
                            EntityAttribute {
                                base: deployment_tier_attr,
                                requirement_level: RequirementLevel::OptIn {
                                    text: "".to_owned(),
                                },
                            },
                            EntityAttribute {
                                base: deployment_region_attr,
                                requirement_level: RequirementLevel::ConditionallyRequired {
                                    text: "When multi-region".to_owned(),
                                },
                            },
                        ],
                        common: CommonFields {
                            brief: "A deployment entity".to_owned(),
                            note: "".to_owned(),
                            stability: Stability::Stable,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    }],
                },
                refinements: Refinements {
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                    entities: vec![],
                },
                dependencies: Default::default(),
                dependency_graph: Default::default(),
            }))
        } else {
            VersionedRegistry::V1(Box::new(ResolvedRegistry {
                registry_url: "TEST_ENTITY".to_owned(),
                groups: vec![
                    // Entity group
                    ResolvedGroup {
                        id: "entity.deployment".to_owned(),
                        r#type: GroupType::Entity,
                        brief: "A deployment entity".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        attributes: vec![
                            Attribute {
                                name: "deployment.name".to_owned(),
                                r#type: AttributeType::PrimitiveOrArray(
                                    PrimitiveOrArrayTypeSpec::String,
                                ),
                                examples: None,
                                brief: "The deployment name".to_owned(),
                                tag: None,
                                requirement_level: RequirementLevel::Basic(
                                    BasicRequirementLevelSpec::Required,
                                ),
                                sampling_relevant: None,
                                note: "".to_owned(),
                                stability: Some(Stability::Stable),
                                deprecated: None,
                                prefix: false,
                                tags: None,
                                value: None,
                                annotations: None,
                                role: Default::default(),
                            },
                            Attribute {
                                name: "deployment.environment".to_owned(),
                                r#type: AttributeType::PrimitiveOrArray(
                                    PrimitiveOrArrayTypeSpec::String,
                                ),
                                examples: None,
                                brief: "The deployment environment".to_owned(),
                                tag: None,
                                requirement_level: RequirementLevel::Recommended {
                                    text: "".to_owned(),
                                },
                                sampling_relevant: None,
                                note: "".to_owned(),
                                stability: Some(Stability::Stable),
                                deprecated: None,
                                prefix: false,
                                tags: None,
                                value: None,
                                annotations: None,
                                role: Default::default(),
                            },
                            Attribute {
                                name: "deployment.tier".to_owned(),
                                r#type: AttributeType::PrimitiveOrArray(
                                    PrimitiveOrArrayTypeSpec::String,
                                ),
                                examples: None,
                                brief: "The deployment tier".to_owned(),
                                tag: None,
                                requirement_level: RequirementLevel::Basic(
                                    BasicRequirementLevelSpec::OptIn,
                                ),
                                sampling_relevant: None,
                                note: "".to_owned(),
                                stability: Some(Stability::Stable),
                                deprecated: None,
                                prefix: false,
                                tags: None,
                                value: None,
                                annotations: None,
                                role: Default::default(),
                            },
                            Attribute {
                                name: "deployment.region".to_owned(),
                                r#type: AttributeType::PrimitiveOrArray(
                                    PrimitiveOrArrayTypeSpec::String,
                                ),
                                examples: None,
                                brief: "The deployment region".to_owned(),
                                tag: None,
                                requirement_level: RequirementLevel::ConditionallyRequired {
                                    text: "When multi-region".to_owned(),
                                },
                                sampling_relevant: None,
                                note: "".to_owned(),
                                stability: Some(Stability::Stable),
                                deprecated: None,
                                prefix: false,
                                tags: None,
                                value: None,
                                annotations: None,
                                role: Default::default(),
                            },
                        ],
                        span_kind: None,
                        events: vec![],
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        requirement_level: None,
                        name: Some("deployment".to_owned()),
                        lineage: None,
                        display_name: None,
                        body: None,
                        annotations: None,
                    },
                    // Event group with entity association
                    ResolvedGroup {
                        id: "event.deployment.started".to_owned(),
                        r#type: GroupType::Event,
                        brief: "A deployment has started".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![EntityAssociation::Ref("deployment".to_owned())],
                        extends: None,
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        attributes: vec![],
                        span_kind: None,
                        events: vec![],
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        requirement_level: None,
                        name: Some("deployment.started".to_owned()),
                        lineage: None,
                        display_name: None,
                        body: None,
                        annotations: None,
                    },
                ],
            }))
        }
    }

    fn make_log_sample(event_name: &str, resource_attributes: Vec<SampleAttribute>) -> Sample {
        use crate::sample_log::SampleLog;
        use crate::sample_resource::SampleResource;

        let resource = Rc::new(SampleResource {
            attributes: resource_attributes,
            live_check_result: None,
        });
        Sample::Log(SampleLog {
            event_name: event_name.to_owned(),
            severity_number: None,
            severity_text: None,
            body: None,
            attributes: vec![],
            trace_id: None,
            span_id: None,
            instrumentation_scope: None,
            live_check_result: None,
            resource: Some(resource),
        })
    }

    fn run_entity_validation_test(use_v2: bool) {
        let registry = make_entity_registry(use_v2);
        let advisors: Vec<Box<dyn Advisor>> = vec![Box::new(TypeAdvisor)];
        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));

        // Case 1: both entity attributes missing from resource
        let mut sample_both_missing = make_log_sample("deployment.started", vec![]);
        sample_both_missing
            .run_live_check(
                &mut live_checker,
                &mut stats,
                None,
                &sample_both_missing.clone(),
            )
            .expect("live check should not error");

        let advice = match &sample_both_missing {
            Sample::Log(log) => log.live_check_result.as_ref().unwrap().all_advice.clone(),
            _ => panic!("expected log sample"),
        };
        assert_eq!(
            advice.len(),
            4,
            "all four entity attributes should produce findings"
        );
        let required_finding = advice
            .iter()
            .find(|a| a.id == "entity_required_attribute_not_present")
            .expect("should have entity_required_attribute_not_present finding");
        assert_eq!(required_finding.level, FindingLevel::Violation);
        assert_eq!(
            required_finding.context,
            Some(json!({"attribute_key": "deployment.name", "entity_type": "deployment"}))
        );
        let recommended_finding = advice
            .iter()
            .find(|a| a.id == "entity_recommended_attribute_not_present")
            .expect("should have entity_recommended_attribute_not_present finding");
        assert_eq!(recommended_finding.level, FindingLevel::Improvement);
        assert_eq!(
            recommended_finding.context,
            Some(json!({"attribute_key": "deployment.environment", "entity_type": "deployment"}))
        );
        let opt_in_finding = advice
            .iter()
            .find(|a| a.id == "entity_opt_in_attribute_not_present")
            .expect("should have entity_opt_in_attribute_not_present finding");
        assert_eq!(opt_in_finding.level, FindingLevel::Information);
        let conditional_finding = advice
            .iter()
            .find(|a| a.id == "entity_conditionally_required_attribute_not_present")
            .expect("should have entity_conditionally_required_attribute_not_present finding");
        assert_eq!(conditional_finding.level, FindingLevel::Information);

        // Case 2: required present, recommended missing
        let mut sample_required_only = make_log_sample(
            "deployment.started",
            vec![SampleAttribute {
                name: "deployment.name".to_owned(),
                value: Some(serde_json::json!("my-app")),
                r#type: None,
                live_check_result: None,
            }],
        );
        sample_required_only
            .run_live_check(
                &mut live_checker,
                &mut stats,
                None,
                &sample_required_only.clone(),
            )
            .expect("live check should not error");

        let advice = match &sample_required_only {
            Sample::Log(log) => log.live_check_result.as_ref().unwrap().all_advice.clone(),
            _ => panic!("expected log sample"),
        };
        assert!(
            advice
                .iter()
                .all(|a| a.id != "entity_required_attribute_not_present"),
            "required attribute is present, no violation expected"
        );
        assert!(
            advice
                .iter()
                .any(|a| a.id == "entity_recommended_attribute_not_present"),
            "recommended attribute still missing"
        );

        // Case 3: all entity attributes present — no entity findings
        let mut sample_all_present = make_log_sample(
            "deployment.started",
            vec![
                SampleAttribute {
                    name: "deployment.name".to_owned(),
                    value: Some(serde_json::json!("my-app")),
                    r#type: None,
                    live_check_result: None,
                },
                SampleAttribute {
                    name: "deployment.environment".to_owned(),
                    value: Some(serde_json::json!("production")),
                    r#type: None,
                    live_check_result: None,
                },
                SampleAttribute {
                    name: "deployment.tier".to_owned(),
                    value: Some(serde_json::json!("frontend")),
                    r#type: None,
                    live_check_result: None,
                },
                SampleAttribute {
                    name: "deployment.region".to_owned(),
                    value: Some(serde_json::json!("us-east-1")),
                    r#type: None,
                    live_check_result: None,
                },
            ],
        );
        sample_all_present
            .run_live_check(
                &mut live_checker,
                &mut stats,
                None,
                &sample_all_present.clone(),
            )
            .expect("live check should not error");

        let advice = match &sample_all_present {
            Sample::Log(log) => log.live_check_result.as_ref().unwrap().all_advice.clone(),
            _ => panic!("expected log sample"),
        };
        assert!(
            advice.iter().all(|a| !a.id.starts_with("entity_")),
            "no entity findings expected when all attributes present"
        );

        // Case 4: no resource — entity findings still emitted (treated as empty resource)
        let mut sample_no_resource = Sample::Log(SampleLog {
            event_name: "deployment.started".to_owned(),
            severity_number: None,
            severity_text: None,
            body: None,
            attributes: vec![],
            trace_id: None,
            span_id: None,
            instrumentation_scope: None,
            live_check_result: None,
            resource: None,
        });
        sample_no_resource
            .run_live_check(
                &mut live_checker,
                &mut stats,
                None,
                &sample_no_resource.clone(),
            )
            .expect("live check should not error");

        let advice = match &sample_no_resource {
            Sample::Log(log) => log.live_check_result.as_ref().unwrap().all_advice.clone(),
            _ => panic!("expected log sample"),
        };
        assert_eq!(
            advice.len(),
            4,
            "entity findings should still be emitted when no resource is present"
        );
        assert!(
            advice
                .iter()
                .any(|a| a.id == "entity_required_attribute_not_present"),
            "required entity attribute finding expected even without resource"
        );
        assert!(
            advice
                .iter()
                .any(|a| a.id == "entity_opt_in_attribute_not_present"),
            "opt-in entity attribute finding expected even without resource"
        );
        assert!(
            advice
                .iter()
                .any(|a| a.id == "entity_conditionally_required_attribute_not_present"),
            "conditionally required entity attribute finding expected even without resource"
        );
    }

    #[test]
    fn test_custom_rego_can_inspect_span_instrumentation_scope() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let policy_path = temp_dir.path().join("scope.rego");
        let rego_content = r#"
            package live_check_advice

            import rego.v1

            make_advice(advice_type, advice_level, advice_context, message) := {
                "type": "advice",
                "advice_type": advice_type,
                "advice_level": advice_level,
                "advice_context": advice_context,
                "message": message,
            }

            deny contains make_advice(advice_type, advice_level, advice_context, message) if {
                input.instrumentation_scope.name == "framework"
                input.instrumentation_scope.version == "1.2.3"
                input.instrumentation_scope.schema_url == "https://opentelemetry.io/schemas/1.32.0"
                input.instrumentation_scope.attributes[0].name == "scope.environment"
                input.instrumentation_scope.dropped_attributes_count == 2
                input.resource != null
                advice_type := "instrumentation_scope_seen"
                advice_level := "information"
                advice_context := {"scope_name": "framework"}
                message := "Instrumentation scope is policy-visible"
            }

            deny contains make_advice(advice_type, advice_level, advice_context, message) if {
                input.instrumentation_scope == null
                advice_type := "instrumentation_scope_absent"
                advice_level := "information"
                advice_context := {"scope_name": null}
                message := "Missing instrumentation scope is explicitly null"
            }
        "#;
        std::fs::write(&policy_path, rego_content).expect("Failed to write custom policy");

        let registry = make_registry(false);
        let mut live_checker = LiveChecker::new(Arc::new(registry), vec![]);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some(temp_dir.path().to_path_buf()),
            &None,
            &None,
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut sample = Sample::Span(SampleSpan {
            name: "operation".to_owned(),
            kind: SpanKindSpec::Internal,
            status: None,
            attributes: vec![],
            span_events: vec![],
            span_links: vec![],
            instrumentation_scope: Some(Rc::new(SampleInstrumentationScope {
                name: "framework".to_owned(),
                version: "1.2.3".to_owned(),
                schema_url: "https://opentelemetry.io/schemas/1.32.0".to_owned(),
                attributes: vec![SampleAttribute {
                    name: "scope.environment".to_owned(),
                    value: Some(json!("test")),
                    r#type: None,
                    live_check_result: None,
                }],
                dropped_attributes_count: 2,
                live_check_result: None,
            })),
            live_check_result: None,
            resource: Some(Rc::new(SampleResource {
                attributes: vec![],
                live_check_result: None,
            })),
        });
        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));

        sample
            .run_live_check(&mut live_checker, &mut stats, None, &sample.clone())
            .expect("live check should not error");

        let advice = match &sample {
            Sample::Span(span) => {
                &span
                    .live_check_result
                    .as_ref()
                    .expect("span should contain a live check result")
                    .all_advice
            }
            _ => unreachable!("test constructs a span"),
        };
        assert!(
            advice
                .iter()
                .any(|finding| finding.id == "instrumentation_scope_seen"),
            "expected custom policy to inspect instrumentation scope: {advice:?}"
        );

        let mut unscoped_sample = sample.clone();
        match &mut unscoped_sample {
            Sample::Span(span) => {
                span.instrumentation_scope = None;
                span.live_check_result = None;
            }
            _ => unreachable!("test constructs a span"),
        }
        unscoped_sample
            .run_live_check(
                &mut live_checker,
                &mut stats,
                None,
                &unscoped_sample.clone(),
            )
            .expect("unscoped live check should not error");
        let unscoped_advice = match &unscoped_sample {
            Sample::Span(span) => &span.live_check_result.as_ref().unwrap().all_advice,
            _ => unreachable!("test constructs a span"),
        };
        assert!(
            unscoped_advice
                .iter()
                .any(|finding| finding.id == "instrumentation_scope_absent"),
            "expected missing scope to be explicitly null for Rego: {unscoped_advice:?}"
        );
    }

    /// Builds a minimal required-only string attribute for entity-association tests.
    fn required_string_attr(name: &str) -> Attribute {
        Attribute {
            name: name.to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: None,
            brief: String::new(),
            tag: None,
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            sampling_relevant: None,
            note: String::new(),
            stability: Some(Stability::Stable),
            deprecated: None,
            prefix: false,
            tags: None,
            value: None,
            annotations: None,
            role: Default::default(),
        }
    }

    /// Builds a V1 entity group with a single required identity attribute.
    fn entity_group(type_name: &str, attr: Attribute) -> ResolvedGroup {
        ResolvedGroup {
            id: format!("entity.{type_name}"),
            r#type: GroupType::Entity,
            brief: String::new(),
            note: String::new(),
            prefix: String::new(),
            entity_associations: vec![],
            extends: None,
            stability: Some(Stability::Stable),
            deprecated: None,
            attributes: vec![attr],
            span_kind: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            requirement_level: None,
            name: Some(type_name.to_owned()),
            lineage: None,
            display_name: None,
            body: None,
            annotations: None,
        }
    }

    /// Builds a V1 event group carrying the given entity associations.
    fn assoc_event_group(name: &str, associations: Vec<EntityAssociation>) -> ResolvedGroup {
        ResolvedGroup {
            id: format!("event.{name}"),
            r#type: GroupType::Event,
            brief: String::new(),
            note: String::new(),
            prefix: String::new(),
            entity_associations: associations,
            extends: None,
            stability: Some(Stability::Stable),
            deprecated: None,
            attributes: vec![],
            span_kind: None,
            events: vec![],
            metric_name: None,
            instrument: None,
            unit: None,
            requirement_level: None,
            name: Some(name.to_owned()),
            lineage: None,
            display_name: None,
            body: None,
            annotations: None,
        }
    }

    fn string_sample_attr(name: &str, value: &str) -> SampleAttribute {
        SampleAttribute {
            name: name.to_owned(),
            value: Some(serde_json::json!(value)),
            r#type: None,
            live_check_result: None,
        }
    }

    /// Runs live-check on a single log event and returns the accumulated findings.
    fn run_event_check(
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        event_name: &str,
        attrs: Vec<SampleAttribute>,
    ) -> Vec<PolicyFinding> {
        let mut sample = make_log_sample(event_name, attrs);
        let snapshot = sample.clone();
        sample
            .run_live_check(live_checker, stats, None, &snapshot)
            .expect("live check should not error");
        match &sample {
            Sample::Log(log) => log.live_check_result.as_ref().unwrap().all_advice.clone(),
            _ => panic!("expected log sample"),
        }
    }

    #[test]
    fn test_entity_association_one_of_all_of() {
        // host (required host.name) and container (required container.id) entities, plus events
        // associated with a `one_of` and an `all_of` of those two entities.
        let registry = VersionedRegistry::V1(Box::new(ResolvedRegistry {
            registry_url: "TEST_ASSOC".to_owned(),
            groups: vec![
                entity_group("host", required_string_attr("host.name")),
                entity_group("container", required_string_attr("container.id")),
                assoc_event_group(
                    "one_of.evt",
                    vec![EntityAssociation::OneOf {
                        one_of: vec![
                            EntityAssociation::Ref("host".to_owned()),
                            EntityAssociation::Ref("container".to_owned()),
                        ],
                    }],
                ),
                assoc_event_group(
                    "all_of.evt",
                    vec![EntityAssociation::AllOf {
                        all_of: vec![
                            EntityAssociation::Ref("host".to_owned()),
                            EntityAssociation::Ref("container".to_owned()),
                        ],
                    }],
                ),
            ],
        }));
        let advisors: Vec<Box<dyn Advisor>> = vec![Box::new(TypeAdvisor)];
        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));

        // one_of satisfied by host → no entity findings at all.
        let advice = run_event_check(
            &mut live_checker,
            &mut stats,
            "one_of.evt",
            vec![string_sample_attr("host.name", "h1")],
        );
        assert!(
            advice.iter().all(|a| !a.id.starts_with("entity_")),
            "one_of satisfied: expected no entity findings, got {advice:?}"
        );

        // one_of unsatisfied → exactly one aggregate finding, no per-branch required findings.
        let advice = run_event_check(&mut live_checker, &mut stats, "one_of.evt", vec![]);
        let aggregates: Vec<_> = advice
            .iter()
            .filter(|a| a.id == "entity_association_not_satisfied")
            .collect();
        assert_eq!(
            aggregates.len(),
            1,
            "one_of unsatisfied: expected a single aggregate finding, got {advice:?}"
        );
        assert_eq!(aggregates[0].level, FindingLevel::Violation);
        assert_eq!(
            aggregates[0].context,
            Some(json!({ "entity_type": ["host", "container"] }))
        );
        assert!(
            advice
                .iter()
                .all(|a| a.id != "entity_required_attribute_not_present"),
            "one_of unsatisfied: expected no per-branch required findings, got {advice:?}"
        );

        // all_of with only host present → container's required attribute is a violation, and no
        // aggregate finding (all_of reports per-attribute).
        let advice = run_event_check(
            &mut live_checker,
            &mut stats,
            "all_of.evt",
            vec![string_sample_attr("host.name", "h1")],
        );
        assert!(
            advice
                .iter()
                .any(|a| a.id == "entity_required_attribute_not_present"
                    && a.context
                        .as_ref()
                        .is_some_and(|c| c["entity_type"] == "container")),
            "all_of: expected a container required-attribute violation, got {advice:?}"
        );
        assert!(
            advice
                .iter()
                .all(|a| a.id != "entity_association_not_satisfied"),
            "all_of: no aggregate finding expected, got {advice:?}"
        );

        // all_of fully satisfied → no entity findings.
        let advice = run_event_check(
            &mut live_checker,
            &mut stats,
            "all_of.evt",
            vec![
                string_sample_attr("host.name", "h1"),
                string_sample_attr("container.id", "c1"),
            ],
        );
        assert!(
            advice.iter().all(|a| !a.id.starts_with("entity_")),
            "all_of satisfied: expected no entity findings, got {advice:?}"
        );
    }

    #[test]
    fn test_entity_association_multiple_top_level_all_of() {
        // Two `all_of` groups at the top level. The top-level list is an implicit `one_of`, so the
        // telemetry must satisfy at least one of the two groups:
        //   - all_of[tenant, host]
        //   - all_of[container]
        let registry = VersionedRegistry::V1(Box::new(ResolvedRegistry {
            registry_url: "TEST_ASSOC_MULTI".to_owned(),
            groups: vec![
                entity_group("host", required_string_attr("host.name")),
                entity_group("container", required_string_attr("container.id")),
                entity_group("tenant", required_string_attr("tenant.id")),
                assoc_event_group(
                    "multi.evt",
                    vec![
                        EntityAssociation::AllOf {
                            all_of: vec![
                                EntityAssociation::Ref("tenant".to_owned()),
                                EntityAssociation::Ref("host".to_owned()),
                            ],
                        },
                        EntityAssociation::AllOf {
                            all_of: vec![EntityAssociation::Ref("container".to_owned())],
                        },
                    ],
                ),
            ],
        }));
        let advisors: Vec<Box<dyn Advisor>> = vec![Box::new(TypeAdvisor)];
        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));

        // Satisfies the second group (container.id present) but not the first → overall satisfied
        // via the implicit one_of, so no aggregate and no per-branch required violations.
        let advice = run_event_check(
            &mut live_checker,
            &mut stats,
            "multi.evt",
            vec![string_sample_attr("container.id", "c1")],
        );
        assert!(
            advice.iter().all(|a| !a.id.starts_with("entity_")),
            "one all_of group satisfied: expected no entity findings, got {advice:?}"
        );

        // Satisfies the first group fully (tenant.id + host.name) → also satisfied.
        let advice = run_event_check(
            &mut live_checker,
            &mut stats,
            "multi.evt",
            vec![
                string_sample_attr("tenant.id", "t1"),
                string_sample_attr("host.name", "h1"),
            ],
        );
        assert!(
            advice.iter().all(|a| !a.id.starts_with("entity_")),
            "other all_of group satisfied: expected no entity findings, got {advice:?}"
        );

        // Satisfies neither group → single aggregate finding naming every candidate entity.
        let advice = run_event_check(&mut live_checker, &mut stats, "multi.evt", vec![]);
        let aggregates: Vec<_> = advice
            .iter()
            .filter(|a| a.id == "entity_association_not_satisfied")
            .collect();
        assert_eq!(
            aggregates.len(),
            1,
            "neither group satisfied: expected a single aggregate finding, got {advice:?}"
        );
        assert_eq!(aggregates[0].level, FindingLevel::Violation);
        let mut entities: Vec<&str> = aggregates[0].context.as_ref().expect("context")
            ["entity_type"]
            .as_array()
            .expect("entity_type array")
            .iter()
            .map(|v| v.as_str().expect("string"))
            .collect();
        entities.sort_unstable();
        assert_eq!(
            entities,
            vec!["container", "host", "tenant"],
            "aggregate should list every candidate entity across both groups"
        );
        assert!(
            advice
                .iter()
                .all(|a| a.id != "entity_required_attribute_not_present"),
            "neither group satisfied: per-branch required findings should be suppressed, got {advice:?}"
        );
    }

    /// Builds the common fields of a stable v2 signal.
    fn v2_common() -> CommonFields {
        CommonFields {
            brief: String::new(),
            note: String::new(),
            stability: Stability::Stable,
            deprecated: None,
            annotations: BTreeMap::new(),
        }
    }

    /// Builds a v2 entity with a single required identity attribute.
    fn v2_entity(entity_type: &str, attr_key: &str) -> V2Entity {
        V2Entity {
            requirement_level: None,
            r#type: SignalId::from(entity_type.to_owned()),
            identity: vec![EntityAttribute {
                base: V2Attribute {
                    key: attr_key.to_owned(),
                    r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                    examples: None,
                    common: v2_common(),
                    provenance: Default::default(),
                },
                requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            }],
            description: vec![],
            common: v2_common(),
            provenance: Default::default(),
        }
    }

    /// The same entity, carrying one annotation.
    fn annotated(mut entity: V2Entity, key: &str, value: &str) -> V2Entity {
        let _ = entity.common.annotations.insert(
            key.to_owned(),
            YamlValue(serde_yaml::Value::String(value.to_owned())),
        );
        entity
    }

    /// Builds a v2 event carrying the given entity associations.
    fn v2_assoc_event(name: &str, associations: Vec<V2EntityAssociation>) -> V2Event {
        V2Event {
            requirement_level: None,
            name: name.to_owned().into(),
            attributes: vec![],
            entity_associations: associations,
            common: v2_common(),
            provenance: Default::default(),
        }
    }

    /// Builds a v2 registry from the pieces an association test needs.
    fn v2_assoc_registry(
        schema_url: &str,
        events: Vec<V2Event>,
        entities: Vec<V2Entity>,
        entity_refinements: Vec<EntityRefinement>,
        dependencies: Vec<(&str, ForgeDependency)>,
    ) -> ForgeResolvedRegistry {
        ForgeResolvedRegistry {
            schema_url: schema_url.try_into().expect("valid schema url"),
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                metrics: vec![],
                spans: vec![],
                events,
                entities,
            },
            refinements: Refinements {
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: entity_refinements,
            },
            dependencies: dependencies
                .into_iter()
                .map(|(url, dep)| (url.try_into().expect("valid schema url"), dep))
                .collect(),
            dependency_graph: Default::default(),
        }
    }

    /// Builds a dependency registry.
    fn v2_dependency(
        entities: Vec<V2Entity>,
        entity_refinements: Vec<EntityRefinement>,
    ) -> ForgeDependency {
        ForgeDependency {
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities,
            },
            refinements: Refinements {
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: entity_refinements,
            },
        }
    }

    /// A reference to an entity that the registry at `schema_url` defines.
    fn dependency_entity_ref(entity_type: &str, schema_url: &str) -> EntityRef {
        EntityRef {
            r#type: entity_type.to_owned().into(),
            provenance: V2Provenance {
                source: Some(schema_url.try_into().expect("valid schema url")),
                path: None,
            },
        }
    }

    /// Builds a live checker and its statistics for one v2 registry.
    fn v2_live_checker(registry: ForgeResolvedRegistry) -> (LiveChecker, LiveCheckStatistics) {
        let advisors: Vec<Box<dyn Advisor>> = vec![Box::new(TypeAdvisor)];
        let live_checker = LiveChecker::new(
            Arc::new(VersionedRegistry::V2(Box::new(registry))),
            advisors,
        );
        let stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
        (live_checker, stats)
    }

    #[test]
    fn test_entity_association_from_dependency_v2() {
        // A registry does not copy the entities of its dependencies, so the
        // definition of `host` is only reachable through the reference.
        const DEP_URL: &str = "https://example.com/base/1.0.0";
        let dependency = v2_dependency(vec![v2_entity("host", "host.name")], vec![]);
        let event = v2_assoc_event(
            "thing.happened",
            vec![V2EntityAssociation::Ref(dependency_entity_ref(
                "host", DEP_URL,
            ))],
        );
        let (mut live_checker, mut stats) = v2_live_checker(v2_assoc_registry(
            "https://example.com/top/1.0.0",
            vec![event],
            vec![],
            vec![],
            vec![(DEP_URL, dependency)],
        ));

        // The resource misses the required identity attribute of the entity.
        let advice = run_event_check(&mut live_checker, &mut stats, "thing.happened", vec![]);
        assert!(
            advice.iter().any(|a| a.id == "entity_required_attribute_not_present"
                && a.level == FindingLevel::Violation
                && a.context.as_ref().is_some_and(|c| c["entity_type"] == "host"
                    && c["attribute_key"] == "host.name")),
            "expected a required-attribute violation for the host entity of the dependency, got {advice:?}"
        );

        // With the attribute present the association is satisfied.
        let advice = run_event_check(
            &mut live_checker,
            &mut stats,
            "thing.happened",
            vec![string_sample_attr("host.name", "h1")],
        );
        assert!(
            advice.iter().all(|a| !a.id.starts_with("entity_")),
            "expected no entity findings, got {advice:?}"
        );
    }

    #[test]
    fn test_entity_association_names_refinement_v2() {
        // An association names an entity type or the id of a refinement, which
        // share one namespace. Here the leaf names a refinement of a dependency.
        const DEP_URL: &str = "https://example.com/base/1.0.0";
        let refinement = EntityRefinement {
            id: SignalId::from("host.windows".to_owned()),
            entity: v2_entity("host", "host.id"),
        };
        let dependency = v2_dependency(vec![], vec![refinement]);
        let event = v2_assoc_event(
            "thing.happened",
            vec![V2EntityAssociation::Ref(dependency_entity_ref(
                "host.windows",
                DEP_URL,
            ))],
        );
        let (mut live_checker, mut stats) = v2_live_checker(v2_assoc_registry(
            "https://example.com/top/1.0.0",
            vec![event],
            vec![],
            vec![],
            vec![(DEP_URL, dependency)],
        ));

        let advice = run_event_check(&mut live_checker, &mut stats, "thing.happened", vec![]);
        assert!(
            advice
                .iter()
                .any(|a| a.id == "entity_required_attribute_not_present"
                    && a.context
                        .as_ref()
                        .is_some_and(|c| c["attribute_key"] == "host.id")),
            "expected the refinement of the dependency to be checked, got {advice:?}"
        );
    }

    #[test]
    fn test_local_entity_wins_over_a_dependency_v2() {
        // Two registries define `host`, and each reference says which one it
        // means, so the leaf decides which definition is checked.
        const DEP_URL: &str = "https://example.com/base/1.0.0";
        let dependency = v2_dependency(vec![v2_entity("host", "host.id")], vec![]);
        let (mut live_checker, mut stats) = v2_live_checker(v2_assoc_registry(
            "https://example.com/top/1.0.0",
            vec![
                v2_assoc_event(
                    "local.evt",
                    vec![V2EntityAssociation::Ref(EntityRef::local(
                        "host".to_owned().into(),
                    ))],
                ),
                v2_assoc_event(
                    "dependency.evt",
                    vec![V2EntityAssociation::Ref(dependency_entity_ref(
                        "host", DEP_URL,
                    ))],
                ),
            ],
            vec![v2_entity("host", "host.name")],
            vec![],
            vec![(DEP_URL, dependency)],
        ));

        let advice = run_event_check(&mut live_checker, &mut stats, "local.evt", vec![]);
        assert!(
            advice.iter().any(|a| a
                .context
                .as_ref()
                .is_some_and(|c| c["attribute_key"] == "host.name")),
            "the local reference must read the local definition, got {advice:?}"
        );

        let advice = run_event_check(&mut live_checker, &mut stats, "dependency.evt", vec![]);
        assert!(
            advice.iter().any(|a| a
                .context
                .as_ref()
                .is_some_and(|c| c["attribute_key"] == "host.id")),
            "the reference into the dependency must read that definition, got {advice:?}"
        );
    }

    /// The data document the default jq preprocessor hands to a Rego policy.
    fn rego_data(live_checker: &LiveChecker) -> serde_json::Value {
        weaver_forge::jq::execute_jq(
            &serde_json::to_value(live_checker).expect("the live checker serializes"),
            crate::DEFAULT_LIVE_CHECK_JQ,
            &BTreeMap::new(),
        )
        .expect("the default jq filter runs")
    }

    /// The entity definitions of one registry, from that data document.
    fn rego_entities_of(
        data: &serde_json::Value,
        schema_url: &str,
    ) -> serde_json::Map<String, serde_json::Value> {
        data["entities"][schema_url]
            .as_object()
            .unwrap_or_else(|| panic!("no entities for {schema_url} in {data}"))
            .clone()
    }

    #[test]
    fn test_rego_data_holds_the_v2_entities() {
        // A Rego policy under live-check cannot look an entity up for itself, so
        // the jq preprocessor derives the definitions from the registry. Both
        // registries define `host`, and each is reachable under its own url.
        const TOP_URL: &str = "https://example.com/top/1.0.0";
        const DEP_URL: &str = "https://example.com/base/1.0.0";
        let dependency = v2_dependency(vec![v2_entity("host", "host.id")], vec![]);
        let (live_checker, _stats) = v2_live_checker(v2_assoc_registry(
            TOP_URL,
            vec![],
            vec![
                v2_entity("service", "service.name"),
                v2_entity("host", "host.name"),
            ],
            vec![EntityRefinement {
                id: SignalId::from("host.windows".to_owned()),
                entity: v2_entity("host", "host.uuid"),
            }],
            vec![(DEP_URL, dependency)],
        ));

        let data = rego_data(&live_checker);
        assert_eq!(
            data["schema_url"], TOP_URL,
            "a leaf with no provenance resolves against this"
        );

        let local = rego_entities_of(&data, TOP_URL);
        let mut names: Vec<&str> = local.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["host", "host.windows", "service"]);
        assert_eq!(
            local["host.windows"]["identity"][0]["key"], "host.uuid",
            "a refinement answers under its id"
        );

        let of_dependency = rego_entities_of(&data, DEP_URL);
        assert_eq!(
            of_dependency.keys().collect::<Vec<_>>(),
            vec!["host"],
            "the dependency keeps its own namespace"
        );
        assert_eq!(
            of_dependency["host"]["identity"][0]["key"], "host.id",
            "the rival definition of `host` is not lost to the local one"
        );
        assert_eq!(local["host"]["identity"][0]["key"], "host.name");
    }

    #[test]
    fn test_rego_data_holds_transitive_entities() {
        // `core` is two hops away: only `base` depends on it.
        const TOP_URL: &str = "https://example.com/top/1.0.0";
        const DEP_URL: &str = "https://example.com/base/1.0.0";
        const CORE_URL: &str = "https://example.com/core/1.0.0";
        let (live_checker, _stats) = v2_live_checker(v2_assoc_registry(
            TOP_URL,
            vec![],
            vec![],
            vec![],
            vec![
                (
                    DEP_URL,
                    v2_dependency(vec![v2_entity("host", "host.id")], vec![]),
                ),
                (
                    CORE_URL,
                    v2_dependency(vec![v2_entity("service", "core.service.name")], vec![]),
                ),
            ],
        ));

        let data = rego_data(&live_checker);
        assert_eq!(
            rego_entities_of(&data, CORE_URL)["service"]["identity"][0]["key"],
            "core.service.name"
        );
    }

    #[test]
    fn test_rego_policy_reads_the_entities() {
        // End to end: the default jq preprocessor hands the entity view to a
        // policy, which reads an annotation from the definition of an entity that
        // a dependency holds, and checks the resource against it. Nothing in the
        // input carries that definition. This registry defines a rival `host`, so
        // the leaf's provenance is what decides which annotation applies.
        const DEP_URL: &str = "https://example.com/base/1.0.0";
        let dependency = v2_dependency(
            vec![annotated(
                v2_entity("host", "host.name"),
                "id_prefix",
                "host-",
            )],
            vec![],
        );
        let event = v2_assoc_event(
            "thing.happened",
            vec![V2EntityAssociation::Ref(dependency_entity_ref(
                "host", DEP_URL,
            ))],
        );
        let registry = VersionedRegistry::V2(Box::new(v2_assoc_registry(
            "https://example.com/top/1.0.0",
            vec![event],
            vec![annotated(
                v2_entity("host", "host.name"),
                "id_prefix",
                "local-",
            )],
            vec![],
            vec![(DEP_URL, dependency)],
        )));
        // No advisors, so the only finding under test is the policy's. The
        // built-in association check is not an advisor and still runs.
        let mut live_checker = LiveChecker::new(Arc::new(registry), vec![]);
        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some("data/policies/entity_advice/".into()),
            &None,
            &None,
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));
        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));

        // The resource carries the identity attribute with the wrong prefix.
        let advice = run_event_check(
            &mut live_checker,
            &mut stats,
            "thing.happened",
            vec![string_sample_attr("host.name", "local-1")],
        );
        assert!(
            advice.iter().any(|a| a.id == "unexpected_entity_id_prefix"
                && a.level == FindingLevel::Improvement
                && a.context
                    .as_ref()
                    .is_some_and(|c| c["entity_type"] == "host"
                        && c["attribute_key"] == "host.name"
                        && c["expected"] == "host-")),
            "the policy should read the annotation of the dependency's entity, not \
             the rival of this registry, got {advice:?}"
        );

        // The same signal, with a value the annotation of the dependency allows.
        let advice = run_event_check(
            &mut live_checker,
            &mut stats,
            "thing.happened",
            vec![string_sample_attr("host.name", "host-1")],
        );
        assert!(
            advice.iter().all(|a| a.id != "unexpected_entity_id_prefix"),
            "a value that matches the prefix must not trigger the policy, got {advice:?}"
        );
    }

    #[test]
    fn test_rego_data_holds_no_entities_for_v1() {
        // The view is derived from the v2 registry shape, which a v1 registry
        // does not have, so a v1 policy sees an empty object rather than an error.
        let registry = VersionedRegistry::V1(Box::new(ResolvedRegistry {
            registry_url: "TEST_V1_ENTITIES".to_owned(),
            groups: vec![entity_group("host", required_string_attr("host.name"))],
        }));
        let live_checker = LiveChecker::new(Arc::new(registry), vec![Box::new(TypeAdvisor)]);

        assert!(
            rego_data(&live_checker)["entities"]
                .as_object()
                .expect("an object, empty")
                .is_empty(),
            "a v1 registry has no entity view"
        );
    }

    #[test]
    fn test_metric_entity_validation() {
        run_metric_entity_validation_test(false);
    }

    #[test]
    fn test_metric_entity_validation_v2() {
        run_metric_entity_validation_test(true);
    }

    fn make_metric_entity_registry(use_v2: bool) -> VersionedRegistry {
        // A "host" entity with host.name (Required) associated with metric system.uptime
        if use_v2 {
            use weaver_forge::v2::entity::{Entity as V2Entity, EntityAttribute};
            use weaver_semconv::v2::signal_id::SignalId;

            let host_name_attr = V2Attribute {
                key: "host.name".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: CommonFields {
                    brief: "The host name".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: BTreeMap::new(),
                },
                provenance: Default::default(),
            };

            VersionedRegistry::V2(Box::new(ForgeResolvedRegistry {
                schema_url: "https://example.com/schemas/1.0.0"
                    .try_into()
                    .expect("valid schema url"),
                registry: Registry {
                    attributes: vec![host_name_attr.clone()],
                    attribute_groups: vec![],
                    metrics: vec![V2Metric {
                        name: "system.uptime".to_owned().into(),
                        instrument: InstrumentSpec::Gauge,
                        unit: "s".to_owned(),
                        requirement_level: None,
                        attributes: vec![],
                        entity_associations: vec![V2EntityAssociation::Ref(EntityRef::local(
                            "host".to_owned().into(),
                        ))],
                        common: CommonFields {
                            brief: "System uptime".to_owned(),
                            note: "".to_owned(),
                            stability: Stability::Stable,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    }],
                    spans: vec![],
                    events: vec![],
                    entities: vec![V2Entity {
                        requirement_level: None,
                        r#type: SignalId::from("host".to_owned()),
                        identity: vec![EntityAttribute {
                            base: host_name_attr,
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                        }],
                        description: vec![],
                        common: CommonFields {
                            brief: "A host entity".to_owned(),
                            note: "".to_owned(),
                            stability: Stability::Stable,
                            deprecated: None,
                            annotations: BTreeMap::new(),
                        },
                        provenance: Default::default(),
                    }],
                },
                refinements: Refinements {
                    metrics: vec![],
                    spans: vec![],
                    events: vec![],
                    entities: vec![],
                },
                dependencies: Default::default(),
                dependency_graph: Default::default(),
            }))
        } else {
            VersionedRegistry::V1(Box::new(ResolvedRegistry {
                registry_url: "TEST_METRIC_ENTITY".to_owned(),
                groups: vec![
                    ResolvedGroup {
                        id: "entity.host".to_owned(),
                        r#type: GroupType::Entity,
                        brief: "A host entity".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![],
                        extends: None,
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        attributes: vec![Attribute {
                            name: "host.name".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::String,
                            ),
                            examples: None,
                            brief: "The host name".to_owned(),
                            tag: None,
                            requirement_level: RequirementLevel::Basic(
                                BasicRequirementLevelSpec::Required,
                            ),
                            sampling_relevant: None,
                            note: "".to_owned(),
                            stability: Some(Stability::Stable),
                            deprecated: None,
                            prefix: false,
                            tags: None,
                            value: None,
                            annotations: None,
                            role: Default::default(),
                        }],
                        span_kind: None,
                        events: vec![],
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        requirement_level: None,
                        name: Some("host".to_owned()),
                        lineage: None,
                        display_name: None,
                        body: None,
                        annotations: None,
                    },
                    ResolvedGroup {
                        id: "metric.system.uptime".to_owned(),
                        r#type: GroupType::Metric,
                        brief: "System uptime".to_owned(),
                        note: "".to_owned(),
                        prefix: "".to_owned(),
                        entity_associations: vec![EntityAssociation::Ref("host".to_owned())],
                        extends: None,
                        stability: Some(Stability::Stable),
                        deprecated: None,
                        attributes: vec![],
                        span_kind: None,
                        events: vec![],
                        metric_name: Some("system.uptime".to_owned()),
                        instrument: Some(InstrumentSpec::Gauge),
                        unit: Some("s".to_owned()),
                        requirement_level: None,
                        name: None,
                        lineage: None,
                        display_name: None,
                        body: None,
                        annotations: None,
                    },
                ],
            }))
        }
    }

    fn run_metric_entity_validation_test(use_v2: bool) {
        use crate::sample_metric::{DataPoints, SampleMetric, SampleNumberDataPoint};
        use crate::sample_resource::SampleResource;

        let registry = make_metric_entity_registry(use_v2);
        let advisors: Vec<Box<dyn Advisor>> = vec![Box::new(TypeAdvisor)];
        let mut live_checker = LiveChecker::new(Arc::new(registry), advisors);
        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));

        let make_metric = |resource_attributes: Vec<SampleAttribute>| {
            let resource = Rc::new(SampleResource {
                attributes: resource_attributes,
                live_check_result: None,
            });
            Sample::Metric(SampleMetric {
                name: "system.uptime".to_owned(),
                unit: "s".to_owned(),
                instrument: SampleInstrument::Supported(InstrumentSpec::Gauge),
                data_points: Some(DataPoints::Number(vec![SampleNumberDataPoint {
                    attributes: vec![],
                    value: serde_json::json!(42.0),
                    flags: 0,
                    exemplars: vec![],
                    live_check_result: None,
                }])),
                instrumentation_scope: None,
                live_check_result: None,
                resource: Some(resource),
            })
        };

        // Resource missing host.name — expect entity_required_attribute_not_present on the metric
        let mut sample_missing = make_metric(vec![]);
        sample_missing
            .run_live_check(&mut live_checker, &mut stats, None, &sample_missing.clone())
            .expect("live check should not error");

        let advice = match &sample_missing {
            Sample::Metric(m) => m.live_check_result.as_ref().unwrap().all_advice.clone(),
            _ => panic!("expected metric sample"),
        };
        assert!(
            advice
                .iter()
                .any(|a| a.id == "entity_required_attribute_not_present"),
            "expected entity_required_attribute_not_present when host.name absent from resource"
        );

        // Resource with host.name — no entity findings
        let mut sample_present = make_metric(vec![SampleAttribute {
            name: "host.name".to_owned(),
            value: Some(serde_json::json!("my-host")),
            r#type: None,
            live_check_result: None,
        }]);
        sample_present
            .run_live_check(&mut live_checker, &mut stats, None, &sample_present.clone())
            .expect("live check should not error");

        let advice = match &sample_present {
            Sample::Metric(m) => m.live_check_result.as_ref().unwrap().all_advice.clone(),
            _ => panic!("expected metric sample"),
        };
        assert!(
            advice
                .iter()
                .all(|a| a.id != "entity_required_attribute_not_present"),
            "no entity findings expected when host.name present in resource"
        );
    }

    #[test]
    fn test_live_checker_loads_only_custom_policies() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let policy_path = temp_path.join("custom.rego");

        let rego_content = r#"
            package live_check_advice

            import rego.v1

            make_advice(advice_type, advice_level, advice_context, message) := {
                "type": "advice",
                "advice_type": advice_type,
                "advice_level": advice_level,
                "advice_context": advice_context,
                "message": message,
            }

            deny contains make_advice(advice_type, advice_level, advice_context, message) if {
                input.sample.attribute
                input.sample.attribute.name == "test.custom_trigger"
                advice_type := "custom_advice_finding"
                advice_level := "violation"
                advice_context := {"trigger": "test.custom_trigger"}
                message := "Custom advisor rule triggered"
            }
        "#;
        std::fs::write(&policy_path, rego_content).expect("Failed to write custom policy");

        let registry = make_registry(false);
        let mut live_checker = LiveChecker::new(Arc::new(registry), vec![]);

        let rego_advisor =
            RegoAdvisor::new(&live_checker, &Some(temp_path.to_path_buf()), &None, &None)
                .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut sample =
            Sample::Attribute(SampleAttribute::try_from("test.custom_trigger=val").unwrap());
        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));

        let result = sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
        assert!(result.is_ok());

        let advice = get_all_advice(&mut sample);
        assert!(!advice.is_empty(), "Expected advice findings");

        let has_default_advice = advice.iter().any(|a| a.id == "missing_attribute");
        assert!(
            has_default_advice,
            "Expected default missing_attribute advice to be present. Found: {:?}",
            advice
        );

        let has_custom_advice = advice.iter().any(|a| a.id == "custom_advice_finding");
        assert!(
            has_custom_advice,
            "Expected custom_advice_finding to be present. Found: {:?}",
            advice
        );
    }

    #[test]
    fn test_live_checker_loads_advice_data() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let policy_dir = temp_path.join("policies");
        let _ = std::fs::create_dir_all(&policy_dir);
        let data_dir = temp_path.join("data");
        let _ = std::fs::create_dir_all(&data_dir);

        let policy_path = policy_dir.join("custom.rego");
        let schema_path = data_dir.join("user.json");

        let rego_content = r#"
            package live_check_advice

            import rego.v1

            make_advice(advice_type, advice_level, advice_context, message) := {
                "type": "advice",
                "advice_type": advice_type,
                "advice_level": advice_level,
                "advice_context": advice_context,
                "message": message,
            }

            deny contains make_advice(advice_type, advice_level, advice_context, message) if {
                input.sample.attribute
                input.sample.attribute.name == "test.custom_trigger"
                data.user.properties.user_id.type == "number"
                advice_type := "custom_advice_finding"
                advice_level := "violation"
                advice_context := {"trigger": "test.custom_trigger"}
                message := "Custom advisor rule triggered with data"
            }
        "#;
        std::fs::write(&policy_path, rego_content).expect("Failed to write custom policy");

        let json_content = r#"
            {
                "properties": {
                    "user_id": {
                        "type": "number"
                    }
                }
            }
        "#;
        std::fs::write(&schema_path, json_content).expect("Failed to write schema json");

        let registry = make_registry(false);
        let mut live_checker = LiveChecker::new(Arc::new(registry), vec![]);

        let rego_advisor = RegoAdvisor::new(
            &live_checker,
            &Some(policy_dir),
            &None,
            &Some(format!("{}/**/*.json", data_dir.to_str().unwrap())),
        )
        .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut sample =
            Sample::Attribute(SampleAttribute::try_from("test.custom_trigger=val").unwrap());
        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));

        let result = sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
        assert!(result.is_ok());

        let advice = get_all_advice(&mut sample);
        assert!(!advice.is_empty(), "Expected advice findings");

        let has_custom_advice = advice.iter().any(|a| a.id == "custom_advice_finding");
        assert!(
            has_custom_advice,
            "Expected custom_advice_finding to be present. Found: {:?}",
            advice
        );
    }

    #[test]
    fn test_live_checker_loads_advice_data_with_glob() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let policy_dir = temp_path.join("policies");
        let _ = std::fs::create_dir_all(&policy_dir);
        let data_dir = temp_path.join("data");
        let _ = std::fs::create_dir_all(&data_dir);

        let policy_path = policy_dir.join("custom.rego");
        let schema_path_load = data_dir.join("user.json");
        let schema_path_skip = data_dir.join("admin.txt");

        let rego_content = r#"
            package live_check_advice

            import rego.v1

            make_advice(advice_type, advice_level, advice_context, message) := {
                "type": "advice",
                "advice_type": advice_type,
                "advice_level": advice_level,
                "advice_context": advice_context,
                "message": message,
            }

            deny contains make_advice(advice_type, advice_level, advice_context, message) if {
                input.sample.attribute
                input.sample.attribute.name == "test.custom_trigger"
                data.user.properties.user_id.type == "number"
                advice_type := "custom_advice_finding"
                advice_level := "violation"
                advice_context := {"trigger": "test.custom_trigger"}
                message := "Custom advisor rule triggered with glob data"
            }
        "#;
        std::fs::write(&policy_path, rego_content).expect("Failed to write custom policy");

        let json_content = r#"
            {
                "properties": {
                    "user_id": {
                        "type": "number"
                    }
                }
            }
        "#;
        std::fs::write(&schema_path_load, json_content).expect("Failed to write schema json");
        std::fs::write(&schema_path_skip, "some dummy text").expect("Failed to write dummy file");

        let registry = make_registry(false);
        let mut live_checker = LiveChecker::new(Arc::new(registry), vec![]);

        let glob_pattern = format!("{}/data/*.json", temp_path.to_str().unwrap());

        let rego_advisor =
            RegoAdvisor::new(&live_checker, &Some(policy_dir), &None, &Some(glob_pattern))
                .expect("Failed to create Rego advisor");
        live_checker.add_advisor(Box::new(rego_advisor));

        let mut sample =
            Sample::Attribute(SampleAttribute::try_from("test.custom_trigger=val").unwrap());
        let mut stats =
            LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));

        let result = sample.run_live_check(&mut live_checker, &mut stats, None, &sample.clone());
        assert!(result.is_ok());

        let advice = get_all_advice(&mut sample);
        assert!(!advice.is_empty(), "Expected advice findings");

        let has_custom_advice = advice.iter().any(|a| a.id == "custom_advice_finding");
        assert!(
            has_custom_advice,
            "Expected custom_advice_finding to be present. Found: {:?}",
            advice
        );
    }

    #[test]
    fn test_rego_advisor_advice_data_invalid_path() {
        let registry = make_registry(false);
        let live_checker = LiveChecker::new(Arc::new(registry), vec![]);

        // A non-existent advice_data path should surface as an AdviceError.
        let result = RegoAdvisor::new(
            &live_checker,
            &None,
            &None,
            &Some("/non/existent/path/*.json".to_owned()),
        );
        assert!(matches!(result, Err(Error::AdviceError { .. })));
    }
}
