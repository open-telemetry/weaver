// SPDX-License-Identifier: Apache-2.0

//! Matchers from the live-check config, compiled and checked at startup.

use std::rc::Rc;

use std::collections::{HashMap, HashSet};

use weaver_cel::{Bindings, Expression, Referenced, Scope};
use weaver_config::live_check::{MatcherConfig, MatcherSampleType};
use weaver_forge::v2::attribute_group::AttributeGroup;

use crate::{
    advice::{
        emit_findings,
        type_advisor::{check_attributes, CheckableAttribute},
        FindingBuilder,
    },
    cel::variables,
    generated::attributes::FindingId,
    live_checker::LiveChecker,
    sample_attribute::SampleAttribute,
    Error, LiveCheckResult, Sample, SampleRef, SampleType, VersionedAttribute, VersionedRegistry,
    VersionedSignal, ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY, SCHEMA_URL_ADVICE_CONTEXT_KEY,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use weaver_checker::FindingLevel;

impl From<MatcherSampleType> for SampleType {
    fn from(sample_type: MatcherSampleType) -> Self {
        match sample_type {
            MatcherSampleType::Span => Self::Span,
            MatcherSampleType::SpanEvent => Self::SpanEvent,
            MatcherSampleType::SpanLink => Self::SpanLink,
            MatcherSampleType::Log => Self::Log,
            MatcherSampleType::Metric => Self::Metric,
            MatcherSampleType::Resource => Self::Resource,
            MatcherSampleType::InstrumentationScope => Self::InstrumentationScope,
            MatcherSampleType::Profile => Self::Profile,
        }
    }
}

/// A matcher from the config, with its `when` compiled.
#[derive(Debug)]
pub struct Matcher {
    /// Identifies the matcher in findings, statistics and coverage.
    pub id: String,

    /// The kind of sample this matcher applies to.
    pub sample_type: SampleType,

    /// The compiled `when`. When `None`, the matcher applies to every sample of
    /// its `sample_type`.
    pub when: Option<Expression>,

    /// The registry signal the sample is checked against.
    pub signal: Option<String>,

    /// Registry attribute groups checked in addition to the signal, in priority
    /// order.
    pub attribute_groups: Vec<String>,

    /// The number of samples the matcher applied to.
    matched: u64,

    /// The number of samples whose `when` errored.
    errors: u64,

    /// The message from the first sample whose `when` errored.
    first_error: Option<String>,
}

/// The configured matchers, in declaration order.
#[derive(Debug, Default)]
pub struct Matchers {
    matchers: Vec<Matcher>,
    /// The variables the matchers of each sample type read, so a sample binds
    /// once.
    referenced: HashMap<SampleType, Referenced>,
}

impl Matchers {
    /// Compiles each `when` and checks it only reads variables its sample type
    /// has.
    ///
    /// # Errors
    ///
    /// Returns an error for a repeated `id`, a `when` that does not compile, or
    /// a `when` that reads a variable the sample type does not have.
    pub fn compile(configs: &[MatcherConfig]) -> Result<Self, Error> {
        let mut matchers: Vec<Matcher> = Vec::with_capacity(configs.len());
        for config in configs {
            if matchers.iter().any(|matcher| matcher.id == config.id) {
                return Err(Error::DuplicateMatcher {
                    id: config.id.clone(),
                });
            }
            let when = config
                .when
                .as_deref()
                .map(|when| compile_when(config, when))
                .transpose()?;
            matchers.push(Matcher {
                id: config.id.clone(),
                sample_type: config.sample_type.into(),
                when,
                signal: config.signal.clone(),
                attribute_groups: config.attribute_groups.clone(),
                matched: 0,
                errors: 0,
                first_error: None,
            });
        }
        let mut referenced: HashMap<SampleType, Referenced> = HashMap::new();
        for matcher in &matchers {
            let _ = referenced.entry(matcher.sample_type).or_insert_with(|| {
                Referenced::union(
                    matchers
                        .iter()
                        .filter(|other| other.sample_type == matcher.sample_type)
                        .filter_map(|other| other.when.as_ref()),
                )
            });
        }
        Ok(Self {
            matchers,
            referenced,
        })
    }

    /// Checks every `signal` and attribute group names something in the
    /// registry.
    ///
    /// # Errors
    ///
    /// Returns an error when the registry is v1, when a sample type that has no
    /// signal sets `signal`, or when a name is not in the registry.
    pub fn check_against(&self, live_checker: &LiveChecker) -> Result<(), Error> {
        let Some(first) = self.matchers.first() else {
            return Ok(());
        };
        if matches!(live_checker.registry.as_ref(), VersionedRegistry::V1(_)) {
            return Err(Error::MatchersRequireV2Registry {
                id: first.id.clone(),
            });
        }
        for matcher in &self.matchers {
            matcher.check_signal(live_checker)?;
            for attribute_group in &matcher.attribute_groups {
                if live_checker.find_attribute_group(attribute_group).is_none() {
                    return Err(Error::UnknownMatcherAttributeGroup {
                        id: matcher.id.clone(),
                        attribute_group: attribute_group.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// The signal and attribute groups to compare a sample with.
    ///
    /// The first matcher with a `signal` overrides the natural match; a later
    /// one is listed in [`SampleMatch::conflicts`]. Attribute groups accumulate
    /// in declaration order, first mention winning.
    pub fn match_for(
        &self,
        sample_type: SampleType,
        bindings: &dyn Bindings,
        natural: Option<Rc<VersionedSignal>>,
        live_checker: &LiveChecker,
    ) -> SampleMatch {
        let mut sample_match = SampleMatch {
            signal: natural,
            ..SampleMatch::default()
        };
        let Some(referenced) = self.referenced.get(&sample_type) else {
            return sample_match;
        };
        let scope = Scope::new(referenced, bindings);
        let mut seen_attribute_groups: Vec<&str> = Vec::new();
        for (index, matcher) in self.matchers.iter().enumerate() {
            if matcher.sample_type != sample_type {
                continue;
            }
            sample_match.targeted = true;
            match matcher.applies_to(&scope) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => {
                    sample_match.errors.push((index, error.to_string()));
                    continue;
                }
            }
            sample_match.applied.push(index);
            if let Some(signal) = &matcher.signal {
                if sample_match.signal_matcher.is_none() {
                    sample_match.signal = live_checker.find_signal(signal, sample_type);
                    sample_match.signal_matcher = Some(matcher.id.clone());
                } else {
                    sample_match.conflicts.push(matcher.id.clone());
                }
            }
            for attribute_group in &matcher.attribute_groups {
                if seen_attribute_groups.contains(&attribute_group.as_str()) {
                    continue;
                }
                seen_attribute_groups.push(attribute_group);
                if let Some(found) = live_checker.find_attribute_group(attribute_group) {
                    sample_match.attribute_groups.push(found);
                }
            }
        }
        sample_match
    }

    /// What a match compared the sample with, by name.
    #[must_use]
    pub fn match_info(&self, sample_match: &SampleMatch, signal_expected: bool) -> MatchInfo {
        MatchInfo {
            signal: sample_match
                .signal
                .as_deref()
                .map(|signal| signal.name().to_owned()),
            signal_matcher: sample_match.signal_matcher.clone(),
            attribute_groups: sample_match
                .attribute_groups
                .iter()
                .map(|attribute_group| attribute_group.id.to_string())
                .collect(),
            entries: sample_match
                .applied
                .iter()
                .filter_map(|index| self.matchers.get(*index))
                .map(|matcher| MatchEntry {
                    matcher: matcher.id.clone(),
                    signal: matcher.signal.clone(),
                    attribute_groups: matcher.attribute_groups.clone(),
                    ignored: sample_match.conflicts.contains(&matcher.id),
                })
                .collect(),
            unmatched: sample_match.is_unmatched(),
            signal_expected,
        }
    }

    /// Counts a match against the matchers that produced it.
    pub fn record_match(&mut self, sample_match: &SampleMatch) {
        for &index in &sample_match.applied {
            if let Some(matcher) = self.matchers.get_mut(index) {
                matcher.matched = matcher.matched.saturating_add(1);
            }
        }
        for (index, message) in &sample_match.errors {
            if let Some(matcher) = self.matchers.get_mut(*index) {
                matcher.record_error(message.clone());
            }
        }
    }

    /// The matchers, in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = &Matcher> {
        self.matchers.iter()
    }

    /// Whether the config declares no matchers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.matchers.is_empty()
    }
}

impl Matcher {
    /// Whether the matcher's `when` passes for this sample.
    ///
    /// A `when` that errors does not match, and returns the error for the
    /// caller to record.
    fn applies_to(&self, scope: &Scope<'_>) -> Result<bool, weaver_cel::Error> {
        let Some(when) = &self.when else {
            return Ok(true);
        };
        when.evaluate_in(scope)
    }

    /// Counts one sample whose `when` errored, keeping the first message.
    fn record_error(&mut self, message: String) {
        self.errors = self.errors.saturating_add(1);
        if self.first_error.is_none() {
            self.first_error = Some(message);
        }
    }

    /// The number of samples the matcher applied to.
    #[must_use]
    pub fn matched(&self) -> u64 {
        self.matched
    }

    /// The number of samples whose `when` errored, with the first message.
    #[must_use]
    pub fn errors(&self) -> Option<(u64, &str)> {
        let message = self.first_error.as_deref()?;
        Some((self.errors, message))
    }

    /// Checks `signal` is allowed for the sample type and names something in
    /// the registry.
    fn check_signal(&self, live_checker: &LiveChecker) -> Result<(), Error> {
        let Some(signal) = &self.signal else {
            return Ok(());
        };
        let Some(kind) = SignalKind::for_sample_type(self.sample_type) else {
            return Err(Error::MatcherSignalNotAllowed {
                id: self.id.clone(),
                sample_type: self.sample_type.to_string(),
            });
        };
        let found = match kind {
            SignalKind::SpanType => live_checker.find_span(signal),
            SignalKind::EventName => live_checker.find_event(signal),
            SignalKind::MetricName => live_checker.find_metric(signal),
        };
        if found.is_none() {
            return Err(Error::UnknownMatcherSignal {
                id: self.id.clone(),
                signal: signal.clone(),
                expected: kind.described().to_owned(),
            });
        }
        Ok(())
    }
}

/// What a sample matched: the signal and attribute groups it is checked
/// against, and the matchers that chose them.
#[derive(Debug, Default)]
pub struct SampleMatch {
    /// The signal the sample is compared with.
    pub signal: Option<Rc<VersionedSignal>>,

    /// The attribute groups added to the match, in priority order.
    pub attribute_groups: Vec<Rc<AttributeGroup>>,

    /// The matcher that set `signal`. `None` when `signal` is the natural
    /// match.
    pub signal_matcher: Option<String>,

    /// Matchers whose `signal` was ignored because `signal_matcher` came first.
    pub conflicts: Vec<String>,

    /// The matchers that applied, as indices into the configured matchers.
    pub applied: Vec<usize>,

    /// The matchers whose `when` errored on this sample, as indices into the
    /// configured matchers, with the message.
    pub errors: Vec<(usize, String)>,

    /// Whether any matcher targets this sample's type.
    pub targeted: bool,
}

impl SampleMatch {
    /// Whether a matcher targets this sample's type and none claimed it.
    ///
    /// A sample type no matcher is written for is never unmatched.
    #[must_use]
    pub fn is_unmatched(&self) -> bool {
        self.targeted && self.applied.is_empty() && self.signal.is_none()
    }

    /// The definition this match holds for an attribute, from the signal or
    /// one of the attribute groups.
    ///
    /// An exact key wins over a template, the signal over the groups, and an
    /// earlier group over a later one.
    #[must_use]
    pub fn find_attribute(
        &self,
        live_checker: &LiveChecker,
        key: &str,
    ) -> Option<Rc<VersionedAttribute>> {
        self.signal
            .as_deref()
            .and_then(|signal| live_checker.find_refined_attribute(signal, key))
            .or_else(|| {
                self.attribute_groups.iter().find_map(|attribute_group| {
                    live_checker.find_attribute_group_attribute(&attribute_group.id, key)
                })
            })
            .or_else(|| {
                self.signal
                    .as_deref()
                    .and_then(|signal| live_checker.find_refined_template(signal, key))
            })
            .or_else(|| {
                self.attribute_groups.iter().find_map(|attribute_group| {
                    live_checker.find_attribute_group_template(&attribute_group.id, key)
                })
            })
    }

    /// Whether the match expects an attribute.
    #[must_use]
    pub fn expects(&self, live_checker: &LiveChecker, key: &str) -> bool {
        self.find_attribute(live_checker, key).is_some()
    }

    /// Whether the match can say which attributes belong on the sample.
    ///
    /// A v2 signal can, whether a matcher or the sample's own name reached it.
    /// A v1 group cannot: its attributes are checked one key at a time against
    /// the whole registry.
    fn holds_attribute_definitions(&self) -> bool {
        !self.attribute_groups.is_empty()
            || matches!(
                self.signal.as_deref(),
                Some(
                    VersionedSignal::Span(_)
                        | VersionedSignal::Metric(_)
                        | VersionedSignal::Event(_)
                )
            )
    }

    /// Adds the findings this match raises to a sample's result.
    ///
    /// `unmatched_sample` is only raised for a sample type some matcher
    /// targets. See [`SampleMatch::is_unmatched`].
    pub fn add_findings(
        &self,
        sample_ref: &SampleRef<'_>,
        attributes: &[SampleAttribute],
        result: &mut LiveCheckResult,
        live_checker: &LiveChecker,
        parent_signal: &Sample,
    ) {
        self.add_attribute_findings(sample_ref, attributes, result, live_checker, parent_signal);
        self.set_match_info(sample_ref, result, live_checker);
    }

    /// Adds the findings this match raises about a set of attributes.
    ///
    /// A metric's attributes are on its data points, so it calls this once per
    /// point.
    pub fn add_attribute_findings(
        &self,
        sample_ref: &SampleRef<'_>,
        attributes: &[SampleAttribute],
        result: &mut LiveCheckResult,
        live_checker: &LiveChecker,
        parent_signal: &Sample,
    ) {
        let emitter = live_checker.otlp_emitter.as_ref().map(|rc| rc.as_ref());
        let holds_definitions = self.holds_attribute_definitions();
        if !self.is_unmatched() {
            for attribute in attributes {
                if self.expects(live_checker, &attribute.name) {
                    continue;
                }
                // The base definition names the schema to reference or import
                // the attribute from.
                let found = live_checker
                    .find_base_attribute(&attribute.name)
                    .or_else(|| live_checker.find_base_template(&attribute.name));
                // Falling through to a dependency's definition is itself
                // unexpected, so it is reported even for a match that names no
                // expected set of its own.
                let only_a_dependency_declares_it = found.is_some_and(|base| !base.declared_here);
                if !holds_definitions && !only_a_dependency_declares_it {
                    continue;
                }
                let context = match found {
                    Some(base) => json!({
                        ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY: attribute.name,
                        SCHEMA_URL_ADVICE_CONTEXT_KEY: base.schema_urls,
                    }),
                    None => json!({ ATTRIBUTE_KEY_ADVICE_CONTEXT_KEY: attribute.name }),
                };
                let defined_in = found
                    .map(|base| format!(" It is defined in {}.", base.schema_urls()))
                    .unwrap_or_default();
                let message = if holds_definitions {
                    format!(
                        "Attribute '{}' is not in the matched signal or attribute groups.{defined_in}",
                        attribute.name
                    )
                } else {
                    format!(
                        "Attribute '{}' is not declared by this registry.{defined_in}",
                        attribute.name
                    )
                };
                let finding = FindingBuilder::new(FindingId::UnexpectedAttribute)
                    .context(context)
                    .message(message)
                    .level(FindingLevel::Improvement)
                    .signal(parent_signal)
                    .build_and_emit(sample_ref, emitter, parent_signal);
                result.add_advice(finding, live_checker.finding_modifier.as_ref(), sample_ref);
            }
        }
        // The type advisor checks the signal's own attributes, so a group's
        // copy of a key the signal or an earlier group declares is skipped
        // here: first mention wins, as in `find_attribute`.
        let mut checked: HashSet<&str> = HashSet::new();
        for attribute_group in &self.attribute_groups {
            let declared = attribute_group.attributes.iter().filter(|attribute| {
                let key = attribute.key();
                !self.signal_declares(live_checker, key) && checked.insert(key)
            });
            let findings = check_attributes(declared, attributes, parent_signal);
            if findings.is_empty() {
                continue;
            }
            emit_findings(&findings, sample_ref, emitter, parent_signal);
            result.add_advice_list(findings, live_checker.finding_modifier.as_ref(), sample_ref);
        }
    }

    /// Whether the matched signal declares this exact key.
    ///
    /// A template the signal declares does not count: an exact key wins over
    /// a template, so the group's copy is the one that applies.
    fn signal_declares(&self, live_checker: &LiveChecker, key: &str) -> bool {
        self.signal
            .as_deref()
            .and_then(|signal| live_checker.find_refined_attribute(signal, key))
            .is_some()
    }

    /// Records what this match compared the sample with.
    ///
    /// A v1 registry takes no matchers, so it reports nothing here.
    pub fn set_match_info(
        &self,
        sample_ref: &SampleRef<'_>,
        result: &mut LiveCheckResult,
        live_checker: &LiveChecker,
    ) {
        if !live_checker.is_v2() {
            return;
        }
        result.match_info = Some(
            live_checker
                .matchers()
                .match_info(self, sample_ref.expects_signal()),
        );
    }
}

/// What one matcher contributed to a sample's match.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MatchEntry {
    /// The matcher.
    pub matcher: String,
    /// The signal it names, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    /// The attribute groups it names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attribute_groups: Vec<String>,
    /// Whether its signal was ignored because one was already set.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ignored: bool,
}

/// What a sample was compared with.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MatchInfo {
    /// The signal, by span type, metric name, event name or v1 group id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    /// The matcher whose `signal` won, absent when the sample's own name
    /// resolved it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal_matcher: Option<String>,
    /// The attribute groups, in priority order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attribute_groups: Vec<String>,
    /// What each matcher that applied contributed, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<MatchEntry>,
    /// Whether a matcher targets this sample's type and none claimed it.
    pub unmatched: bool,
    /// Whether this sample type resolves a signal at all, so that having none
    /// is a gap rather than the shape of the sample.
    pub signal_expected: bool,
}

impl MatchInfo {
    /// Whether the sample was compared with anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.signal.is_none() && self.attribute_groups.is_empty()
    }
}

/// What a matcher's `signal` names, if its sample type has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    /// The `type` of a span.
    SpanType,
    /// The `name` of an event.
    EventName,
    /// The `name` of a metric.
    MetricName,
}

impl SignalKind {
    /// What a matcher of this sample type may name in `signal`, or `None` when
    /// `signal` is not allowed.
    #[must_use]
    pub fn for_sample_type(sample_type: SampleType) -> Option<Self> {
        match sample_type {
            SampleType::Span => Some(Self::SpanType),
            SampleType::SpanEvent | SampleType::Log => Some(Self::EventName),
            SampleType::Metric => Some(Self::MetricName),
            _ => None,
        }
    }

    /// How the kind reads in an error message.
    fn described(self) -> &'static str {
        match self {
            Self::SpanType => "a span type",
            Self::EventName => "an event name",
            Self::MetricName => "a metric name",
        }
    }
}

/// Compiles one `when` and checks the variables it reads.
fn compile_when(config: &MatcherConfig, when: &str) -> Result<Expression, Error> {
    let expression =
        Expression::compile(when).map_err(|error| Error::InvalidMatcherExpression {
            id: config.id.clone(),
            error: error.to_string(),
        })?;
    let available = variables(config.sample_type.into());
    for variable in expression.referenced().variables() {
        if !available.contains(&variable) {
            return Err(Error::UnknownMatcherVariable {
                id: config.id.clone(),
                variable: variable.to_owned(),
                sample_type: config.sample_type.to_string(),
                available: available.join(", "),
            });
        }
    }
    Ok(expression)
}

#[cfg(test)]
pub(crate) mod fixture {
    use serde::Deserialize;
    use weaver_config::live_check::MatcherConfig;

    /// The matchers declared in a `[[live-check.matchers]]` fixture.
    pub(crate) fn matcher_configs(toml_str: &str) -> Vec<MatcherConfig> {
        #[derive(Debug, Deserialize)]
        struct Fixture {
            #[serde(rename = "live-check")]
            live_check: LiveCheck,
        }
        #[derive(Debug, Deserialize)]
        struct LiveCheck {
            matchers: Vec<MatcherConfig>,
        }

        let fixture: Fixture = toml::from_str(toml_str).expect("the fixture config parses");
        fixture.live_check.matchers
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use std::collections::BTreeMap;

    use weaver_forge::registry::ResolvedRegistry;
    use weaver_forge::v2::attribute::Attribute as V2Attribute;
    use weaver_forge::v2::attribute_group::AttributeGroupAttribute;
    use weaver_forge::v2::provenance::Provenance;
    use weaver_forge::v2::registry::{ForgeDependency, ForgeResolvedRegistry};
    use weaver_forge::v2::span::SpanAttribute;
    use weaver_semconv::attribute::{
        AttributeType, BasicRequirementLevelSpec, PrimitiveOrArrayTypeSpec, RequirementLevel,
        TemplateTypeSpec,
    };
    use weaver_semconv::stability::Stability;
    use weaver_semconv::v2::CommonFields;

    use crate::advice::{Advisor, TypeAdvisor};
    use crate::sample_log::SampleLog;

    use super::{fixture::matcher_configs, *};

    /// An empty v1 registry.
    fn v1_live_checker() -> LiveChecker {
        let registry = ResolvedRegistry {
            registry_url: "TEST".to_owned(),
            groups: Vec::new(),
        };
        LiveChecker::new(
            Arc::new(VersionedRegistry::V1(Box::new(registry))),
            Vec::new(),
        )
    }

    /// A v2 registry with one span type, one attribute group, one metric and
    /// one event.
    fn v2_live_checker() -> LiveChecker {
        v2_live_checker_with(Vec::new(), Vec::new())
    }

    /// The fixture registry, with the span carrying `attributes` and the
    /// advisors in `advisors`.
    fn v2_live_checker_with(
        attributes: Vec<SpanAttribute>,
        advisors: Vec<Box<dyn Advisor>>,
    ) -> LiveChecker {
        v2_live_checker_full(Vec::new(), attributes, Vec::new(), advisors)
    }

    /// The attributes the fixture span sample carries, so a test that is not
    /// about `unexpected_attribute` does not raise it.
    fn sample_span_attributes() -> Vec<SpanAttribute> {
        vec![
            span_attribute(
                "myapp.checkout.id",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            ),
            span_attribute(
                "myapp.checkout.stage",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            ),
        ]
    }

    /// The fixture registry, with `catalog` as the registry attributes and the
    /// span carrying `attributes`.
    fn v2_live_checker_full(
        catalog: Vec<V2Attribute>,
        attributes: Vec<SpanAttribute>,
        attribute_group_attributes: Vec<AttributeGroupAttribute>,
        advisors: Vec<Box<dyn Advisor>>,
    ) -> LiveChecker {
        let mut registry: ForgeResolvedRegistry =
            serde_json::from_str(include_str!("../fixtures/registry-v2.json"))
                .expect("the fixture registry parses");
        // Neither `Attribute` nor `SpanAttribute` can be deserialized: both
        // combine `deny_unknown_fields` with `flatten`, which serde does not
        // support.
        registry.registry.attributes = catalog;
        if let Some(span) = registry.registry.spans.first_mut() {
            span.attributes = attributes;
        }
        if let Some(attribute_group) = registry.registry.attribute_groups.first_mut() {
            attribute_group.attributes = attribute_group_attributes;
        }
        LiveChecker::new(
            Arc::new(VersionedRegistry::V2(Box::new(registry))),
            advisors,
        )
    }

    /// A registry attribute for the fixture registry.
    fn base_attribute(key: &str, stability: Stability) -> V2Attribute {
        V2Attribute {
            key: key.to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            examples: None,
            common: CommonFields {
                brief: String::new(),
                note: String::new(),
                stability,
                deprecated: None,
                annotations: BTreeMap::new(),
            },
            provenance: Provenance::default(),
        }
    }

    /// A base template attribute of the fixture registry.
    fn template_attribute(key: &str, stability: Stability) -> V2Attribute {
        V2Attribute {
            r#type: AttributeType::Template(TemplateTypeSpec::String),
            ..base_attribute(key, stability)
        }
    }

    /// A span attribute of the fixture registry.
    fn span_attribute(key: &str, requirement_level: RequirementLevel) -> SpanAttribute {
        SpanAttribute {
            base: base_attribute(key, Stability::Stable),
            requirement_level,
            sampling_relevant: None,
        }
    }

    /// An attribute of the fixture registry's `myapp.common` attribute group.
    fn attribute_group_attribute(key: &str) -> AttributeGroupAttribute {
        AttributeGroupAttribute {
            base: base_attribute(key, Stability::Stable),
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
        }
    }

    /// A span attribute whose stability differs from the base definition.
    fn refined_span_attribute(key: &str, stability: Stability) -> SpanAttribute {
        SpanAttribute {
            base: base_attribute(key, stability),
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            sampling_relevant: None,
        }
    }

    /// Every fixture in `fixtures/cel/`.
    const FIXTURES: [&str; 6] = [
        include_str!("../fixtures/cel/span-checkout/matchers.toml"),
        include_str!("../fixtures/cel/span-status/matchers.toml"),
        include_str!("../fixtures/cel/log-common/matchers.toml"),
        include_str!("../fixtures/cel/metric-common/matchers.toml"),
        include_str!("../fixtures/cel/resource/matchers.toml"),
        include_str!("../fixtures/cel/instrumentation-scope/matchers.toml"),
    ];

    fn compile(toml_str: &str) -> Result<Matchers, Error> {
        Matchers::compile(&matcher_configs(toml_str))
    }

    #[test]
    fn the_fixture_matchers_compile_and_pass_the_lint() {
        for toml_str in FIXTURES {
            let matchers = compile(toml_str).expect("the fixture matchers compile");
            assert!(!matchers.is_empty());
        }
    }

    #[test]
    fn no_matchers_is_not_an_error() {
        let matchers = Matchers::compile(&[]).expect("it compiles");
        assert!(matchers.is_empty());
        assert_eq!(matchers.iter().count(), 0);
    }

    #[test]
    fn matchers_keep_their_declaration_order() {
        let matchers = compile(
            r#"
[[live-check.matchers]]
id = "myapp.first"
sample_type = "span"
when = 'name == "a"'

[[live-check.matchers]]
id = "myapp.second"
sample_type = "log"
"#,
        )
        .expect("they compile");
        let ids: Vec<_> = matchers.iter().map(|matcher| matcher.id.as_str()).collect();
        assert_eq!(ids, ["myapp.first", "myapp.second"]);
    }

    #[test]
    fn a_matcher_without_a_when_compiles_to_no_expression() {
        let matchers = compile(
            r#"
[[live-check.matchers]]
id = "myapp.every.log"
sample_type = "log"
attribute_groups = ["myapp.common"]
"#,
        )
        .expect("it compiles");
        let matcher = matchers.iter().next().expect("there is one matcher");
        assert!(matcher.when.is_none());
        assert_eq!(matcher.sample_type, SampleType::Log);
    }

    #[test]
    fn a_when_that_does_not_parse_is_rejected() {
        let error = compile(
            r#"
[[live-check.matchers]]
id = "myapp.broken"
sample_type = "span"
when = 'attributes['
"#,
        )
        .expect_err("it does not compile");
        assert!(
            matches!(&error, Error::InvalidMatcherExpression { id, .. } if id == "myapp.broken"),
            "{error}"
        );
    }

    #[test]
    fn a_variable_from_another_sample_type_is_rejected() {
        let error = compile(
            r#"
[[live-check.matchers]]
id = "myapp.wrong.type"
sample_type = "span"
when = 'unit == "s"'
"#,
        )
        .expect_err("the lint rejects it");
        let Error::UnknownMatcherVariable {
            id,
            variable,
            sample_type,
            available,
        } = &error
        else {
            panic!("wrong variant: {error}");
        };
        assert_eq!(id, "myapp.wrong.type");
        assert_eq!(variable, "unit");
        assert_eq!(sample_type, "span");
        assert!(available.contains("status"), "{available}");
    }

    #[test]
    fn a_variable_no_sample_type_has_is_rejected() {
        let error = compile(
            r#"
[[live-check.matchers]]
id = "myapp.invented"
sample_type = "metric"
when = 'temperature > 3'
"#,
        )
        .expect_err("the lint rejects it");
        assert!(
            matches!(&error, Error::UnknownMatcherVariable { variable, .. } if variable == "temperature"),
            "{error}"
        );
    }

    /// A resource has neither variable, unlike every signal sample.
    #[test]
    fn resource_and_instrumentation_scope_are_rejected_on_a_resource() {
        for when in [
            r#"resource.attributes["service.name"] == "x""#,
            r#"instrumentation_scope.name == "x""#,
        ] {
            let error = compile(&format!(
                r#"
[[live-check.matchers]]
id = "myapp.resource"
sample_type = "resource"
when = '{when}'
"#
            ))
            .expect_err("the lint rejects it");
            assert!(
                matches!(error, Error::UnknownMatcherVariable { .. }),
                "{error}"
            );
        }
    }

    #[test]
    fn a_repeated_id_is_rejected() {
        let error = compile(
            r#"
[[live-check.matchers]]
id = "myapp.checkout"
sample_type = "span"

[[live-check.matchers]]
id = "myapp.checkout"
sample_type = "log"
"#,
        )
        .expect_err("the id is repeated");
        assert!(
            matches!(&error, Error::DuplicateMatcher { id } if id == "myapp.checkout"),
            "{error}"
        );
    }

    fn check(toml_str: &str) -> Result<(), Error> {
        let matchers = Matchers::compile(&matcher_configs(toml_str)).expect("they compile");
        matchers.check_against(&v2_live_checker())
    }

    #[test]
    fn the_registry_fixture_indexes_a_span_and_an_attribute_group() {
        let live_checker = v2_live_checker();
        assert!(live_checker.find_span("myapp.checkout").is_some());
        assert!(live_checker.find_attribute_group("myapp.common").is_some());
        assert!(live_checker.find_span("myapp.absent").is_none());
        assert!(live_checker.find_attribute_group("myapp.absent").is_none());
    }

    #[test]
    fn searching_all_attributes_needs_a_v2_registry() {
        let error = v1_live_checker()
            .search_all_attributes()
            .expect_err("a v1 registry has nothing to search");
        assert!(
            matches!(error, Error::SearchAllAttributesRequiresV2Registry),
            "{error}"
        );
    }

    #[test]
    fn a_v1_registry_indexes_no_spans_and_no_attribute_groups() {
        let live_checker = v1_live_checker();
        assert!(live_checker.find_span("myapp.checkout").is_none());
        assert!(live_checker.find_attribute_group("myapp.common").is_none());
    }

    #[test]
    fn a_matcher_naming_a_signal_and_a_group_in_the_registry_passes() {
        check(
            r#"
[[live-check.matchers]]
id = "myapp.checkout"
sample_type = "span"
when = '"myapp.checkout.id" in attributes'
signal = "myapp.checkout"
attribute_groups = ["myapp.common"]
"#,
        )
        .expect("it checks out");
    }

    #[test]
    fn a_log_matcher_names_an_event_and_a_metric_matcher_names_a_metric() {
        check(
            r#"
[[live-check.matchers]]
id = "myapp.log"
sample_type = "log"
signal = "myapp.order.placed"

[[live-check.matchers]]
id = "myapp.metric"
sample_type = "metric"
signal = "myapp.checkout.duration"
"#,
        )
        .expect("it checks out");
    }

    #[test]
    fn a_signal_that_is_not_in_the_registry_is_rejected() {
        let error = check(
            r#"
[[live-check.matchers]]
id = "myapp.checkout"
sample_type = "span"
signal = "myapp.absent"
"#,
        )
        .expect_err("the signal is not there");
        let Error::UnknownMatcherSignal {
            id,
            signal,
            expected,
        } = &error
        else {
            panic!("wrong variant: {error}");
        };
        assert_eq!(id, "myapp.checkout");
        assert_eq!(signal, "myapp.absent");
        assert_eq!(expected, "a span type");
    }

    /// A span type is not an event name, so the kind of lookup matters.
    #[test]
    fn a_signal_of_the_wrong_kind_is_rejected() {
        let error = check(
            r#"
[[live-check.matchers]]
id = "myapp.log"
sample_type = "log"
signal = "myapp.checkout"
"#,
        )
        .expect_err("a span type is not an event name");
        assert!(
            matches!(&error, Error::UnknownMatcherSignal { expected, .. } if expected == "an event name"),
            "{error}"
        );
    }

    #[test]
    fn an_attribute_group_that_is_not_in_the_registry_is_rejected() {
        let error = check(
            r#"
[[live-check.matchers]]
id = "myapp.log"
sample_type = "log"
attribute_groups = ["myapp.common", "myapp.absent"]
"#,
        )
        .expect_err("the attribute group is not there");
        assert!(
            matches!(&error, Error::UnknownMatcherAttributeGroup { attribute_group, .. }
                if attribute_group == "myapp.absent"),
            "{error}"
        );
    }

    /// A resource, scope, span link and profile have no signal to name.
    #[test]
    fn a_signal_on_a_sample_type_without_one_is_rejected() {
        for sample_type in ["resource", "instrumentation_scope", "span_link", "profile"] {
            let error = check(&format!(
                r#"
[[live-check.matchers]]
id = "myapp.matcher"
sample_type = "{sample_type}"
signal = "myapp.checkout"
"#
            ))
            .expect_err("signal is not allowed");
            assert!(
                matches!(&error, Error::MatcherSignalNotAllowed { sample_type: got, .. }
                    if got == sample_type),
                "{error}"
            );
        }
    }

    #[test]
    fn matchers_are_rejected_against_a_v1_registry() {
        let matchers = Matchers::compile(&matcher_configs(
            r#"
[[live-check.matchers]]
id = "myapp.checkout"
sample_type = "span"
"#,
        ))
        .expect("it compiles");
        let error = matchers
            .check_against(&v1_live_checker())
            .expect_err("matchers need a v2 registry");
        assert!(
            matches!(&error, Error::MatchersRequireV2Registry { id } if id == "myapp.checkout"),
            "{error}"
        );
    }

    /// Without matchers a v1 registry is untouched.
    #[test]
    fn no_matchers_passes_against_a_v1_registry() {
        Matchers::default()
            .check_against(&v1_live_checker())
            .expect("nothing to check");
    }

    mod sample_match {
        use super::*;
        use crate::{sample_span::SampleSpan, CumulativeStatistics, LiveCheckStatistics};

        fn matchers(toml_str: &str) -> Matchers {
            Matchers::compile(&matcher_configs(toml_str)).expect("they compile")
        }

        fn span(json: &str) -> SampleSpan {
            serde_json::from_str(json).expect("the fixture sample parses")
        }

        fn checkout_span() -> SampleSpan {
            span(include_str!(
                "../fixtures/cel/span-checkout/span-checkout-payment.json"
            ))
        }

        fn other_span() -> SampleSpan {
            span(include_str!(
                "../fixtures/cel/span-checkout/span-no-signature.json"
            ))
        }

        fn match_for(matchers: &Matchers, sample: &SampleSpan) -> SampleMatch {
            let live_checker = v2_live_checker();
            matchers.match_for(SampleType::Span, sample, None, &live_checker)
        }

        /// A live checker holding the matchers, as the sample path uses it.
        fn checker_with(toml_str: &str) -> LiveChecker {
            let mut live_checker = v2_live_checker();
            live_checker
                .set_matchers(&matcher_configs(toml_str))
                .expect("they check out");
            live_checker
        }

        /// Compares a sample and records the errors, as the sample path does.
        fn compare_and_record(live_checker: &mut LiveChecker, sample: &SampleSpan) -> SampleMatch {
            let sample_match = live_checker.match_for(SampleType::Span, sample, None);
            live_checker.record_match(&sample_match);
            sample_match
        }

        /// The one matcher.
        fn only_matcher(live_checker: &LiveChecker) -> &Matcher {
            live_checker
                .matchers()
                .iter()
                .next()
                .expect("there is one matcher")
        }

        /// The errors recorded against the one matcher.
        fn recorded(live_checker: &LiveChecker) -> Option<(u64, String)> {
            only_matcher(live_checker)
                .errors()
                .map(|(count, message)| (count, message.to_owned()))
        }

        const ERRORING: &str = r#"
[[live-check.matchers]]
id = "myapp.errors"
sample_type = "span"
when = 'instrumentation_scope.name == "myapp"'
"#;

        fn span_without_scope() -> SampleSpan {
            let mut span = checkout_span();
            span.instrumentation_scope = None;
            span
        }

        #[test]
        fn a_matched_span_takes_the_signal_its_matcher_names() {
            let matchers = matchers(include_str!("../fixtures/cel/span-checkout/matchers.toml"));
            let sample_match = match_for(&matchers, &checkout_span());
            assert_eq!(sample_match.applied.len(), 1);
            assert_eq!(
                sample_match.signal_matcher.as_deref(),
                Some("myapp.checkout")
            );
            assert!(sample_match.signal.is_some());
            assert!(sample_match.conflicts.is_empty());
        }

        #[test]
        fn a_span_that_matches_nothing_resolves_to_nothing() {
            let matchers = matchers(include_str!("../fixtures/cel/span-checkout/matchers.toml"));
            let sample_match = match_for(&matchers, &other_span());
            assert!(sample_match.applied.is_empty());
            assert!(sample_match.signal.is_none());
            assert!(sample_match.signal_matcher.is_none());
        }

        #[test]
        fn no_matchers_keeps_the_natural_match() {
            let live_checker = v2_live_checker();
            let natural = live_checker.find_span("myapp.checkout");
            let sample_match = Matchers::default().match_for(
                SampleType::Span,
                &checkout_span(),
                natural,
                &live_checker,
            );
            assert!(sample_match.signal.is_some());
            assert!(sample_match.applied.is_empty());
            assert!(sample_match.signal_matcher.is_none());
        }

        #[test]
        fn a_matcher_signal_overrides_the_natural_match() {
            let live_checker = v2_live_checker();
            let matchers = matchers(
                r#"
[[live-check.matchers]]
id = "myapp.override"
sample_type = "span"
signal = "myapp.checkout"
"#,
            );
            let natural = live_checker.find_metric("myapp.checkout.duration");
            let sample_match =
                matchers.match_for(SampleType::Span, &checkout_span(), natural, &live_checker);
            assert_eq!(
                sample_match.signal_matcher.as_deref(),
                Some("myapp.override")
            );
            let signal = sample_match.signal.expect("a signal");
            assert!(matches!(signal.as_ref(), VersionedSignal::Span(_)));
        }

        #[test]
        fn the_first_matcher_with_a_signal_wins_and_the_second_conflicts() {
            let matchers = matchers(
                r#"
[[live-check.matchers]]
id = "myapp.first"
sample_type = "span"
signal = "myapp.checkout"

[[live-check.matchers]]
id = "myapp.second"
sample_type = "span"
signal = "myapp.checkout"
"#,
            );
            let sample_match = match_for(&matchers, &checkout_span());
            assert_eq!(sample_match.applied.len(), 2);
            assert_eq!(sample_match.signal_matcher.as_deref(), Some("myapp.first"));
            assert_eq!(sample_match.conflicts, ["myapp.second"]);
        }

        #[test]
        fn attribute_groups_accumulate_in_declaration_order_without_repeats() {
            let matchers = matchers(
                r#"
[[live-check.matchers]]
id = "myapp.first"
sample_type = "span"
attribute_groups = ["myapp.common"]

[[live-check.matchers]]
id = "myapp.second"
sample_type = "span"
attribute_groups = ["myapp.common"]
"#,
            );
            let sample_match = match_for(&matchers, &checkout_span());
            assert_eq!(sample_match.applied.len(), 2);
            assert_eq!(sample_match.attribute_groups.len(), 1);
            assert!(sample_match.signal.is_none());
        }

        #[test]
        fn a_matcher_for_another_sample_type_does_not_apply() {
            let live_checker = v2_live_checker();
            let matchers = matchers(
                r#"
[[live-check.matchers]]
id = "myapp.log"
sample_type = "log"
attribute_groups = ["myapp.common"]
"#,
            );
            let sample_match =
                matchers.match_for(SampleType::Span, &checkout_span(), None, &live_checker);
            assert!(sample_match.applied.is_empty());
            assert!(sample_match.attribute_groups.is_empty());
        }

        /// A guarded expression the lint passes can still error on a sample.
        #[test]
        fn a_when_that_errors_does_not_match_and_is_counted() {
            let mut live_checker = checker_with(ERRORING);
            let sample_match = compare_and_record(&mut live_checker, &span_without_scope());

            assert!(sample_match.applied.is_empty());
            assert_eq!(sample_match.errors.len(), 1);

            let (count, message) = recorded(&live_checker).expect("it errored");
            assert_eq!(count, 1);
            assert!(message.contains("instrumentation_scope"), "{message}");
        }

        #[test]
        fn the_error_count_covers_every_sample() {
            let mut live_checker = checker_with(ERRORING);
            let span = span_without_scope();
            for _ in 0..3 {
                let _ = compare_and_record(&mut live_checker, &span);
            }
            assert_eq!(recorded(&live_checker).expect("it errored").0, 3);
        }

        const CHECKOUT: &str = include_str!("../fixtures/cel/span-checkout/matchers.toml");

        #[test]
        fn the_match_count_covers_every_sample() {
            let mut live_checker = checker_with(CHECKOUT);
            let span = checkout_span();
            for _ in 0..3 {
                let _ = compare_and_record(&mut live_checker, &span);
            }
            assert_eq!(only_matcher(&live_checker).matched(), 3);
        }

        #[test]
        fn a_matcher_that_applies_to_nothing_counts_no_matches() {
            let mut live_checker = checker_with(CHECKOUT);
            let _ = compare_and_record(&mut live_checker, &other_span());
            assert_eq!(only_matcher(&live_checker).matched(), 0);
        }

        /// A `when` that errors is not a match.
        #[test]
        fn a_matcher_that_only_errors_counts_no_matches() {
            let mut live_checker = checker_with(ERRORING);
            let _ = compare_and_record(&mut live_checker, &span_without_scope());
            let matcher = only_matcher(&live_checker);
            assert_eq!(matcher.matched(), 0);
            assert_eq!(matcher.errors().expect("it errored").0, 1);
        }

        #[test]
        fn the_statistics_carry_what_each_matcher_did() {
            let mut live_checker = checker_with(CHECKOUT);
            let _ = compare_and_record(&mut live_checker, &checkout_span());
            let _ = compare_and_record(&mut live_checker, &other_span());

            let mut stats =
                LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
            stats.finalize(live_checker.matchers());

            let LiveCheckStatistics::Cumulative(stats) = stats else {
                panic!("cumulative statistics");
            };
            assert_eq!(stats.matchers.len(), 1);
            assert_eq!(stats.matchers[0].id, "myapp.checkout");
            assert_eq!(stats.matchers[0].matched, 1);
            assert_eq!(stats.matchers[0].errors, 0);
            assert!(stats.matchers[0].first_error.is_none());
        }

        /// The match a config produces for a span sample.
        fn match_info(toml_str: &str, sample: &SampleSpan) -> MatchInfo {
            let mut live_checker = v2_live_checker_with(sample_span_attributes(), Vec::new());
            live_checker
                .set_matchers(&matcher_configs(toml_str))
                .expect("they check out");
            let sample_match = live_checker.match_for(SampleType::Span, sample, None);
            live_checker.matchers().match_info(&sample_match, true)
        }

        #[test]
        fn a_span_that_matches_nothing_is_unmatched() {
            let info = match_info(
                r#"
[[live-check.matchers]]
id = "myapp.never"
sample_type = "span"
when = 'name == "no-such-span"'
"#,
                &checkout_span(),
            );
            assert!(info.unmatched);
            assert!(info.entries.is_empty());
        }

        /// A span reaches no signal by name, so it has nothing to check against.
        #[test]
        fn no_matchers_raises_no_unmatched_sample() {
            let live_checker = v2_live_checker();
            let sample = checkout_span();
            let sample_match = live_checker.match_for(SampleType::Span, &sample, None);
            assert!(!sample_match.is_unmatched());
            let mut result = LiveCheckResult::new();
            let parent = Sample::Span(sample.clone());
            sample_match.add_findings(
                &SampleRef::Span(&sample),
                &sample.attributes,
                &mut result,
                &live_checker,
                &parent,
            );
            assert!(result.all_advice.is_empty());
        }

        /// The event a log's name resolves to declares what belongs on it.
        #[test]
        fn a_natural_event_match_still_raises_unexpected_attribute() {
            let live_checker = v2_live_checker();
            let sample: SampleLog = serde_json::from_str(
                r#"{ "event_name": "myapp.order.placed",
                     "attributes": [{ "name": "myapp.stray.field", "value": "x" }],
                     "live_check_result": null }"#,
            )
            .expect("the fixture sample parses");
            let natural = live_checker.find_event("myapp.order.placed");
            assert!(natural.is_some(), "the fixture registry declares the event");
            let sample_match = live_checker.match_for(SampleType::Log, &sample, natural);
            let mut result = LiveCheckResult::new();
            let parent = Sample::Log(sample.clone());
            sample_match.add_findings(
                &SampleRef::Log(&sample),
                &sample.attributes,
                &mut result,
                &live_checker,
                &parent,
            );
            let ids: Vec<&str> = result
                .all_advice
                .iter()
                .map(|finding| finding.id.as_str())
                .collect();
            assert_eq!(ids, ["unexpected_attribute"]);
        }

        #[test]
        fn a_second_signal_is_a_conflict() {
            let info = match_info(
                r#"
[[live-check.matchers]]
id = "myapp.first"
sample_type = "span"
signal = "myapp.checkout"

[[live-check.matchers]]
id = "myapp.second"
sample_type = "span"
signal = "myapp.checkout"
"#,
                &checkout_span(),
            );
            assert_eq!(info.signal_matcher.as_deref(), Some("myapp.first"));
            let ignored: Vec<&str> = info
                .entries
                .iter()
                .filter(|entry| entry.ignored)
                .map(|entry| entry.matcher.as_str())
                .collect();
            assert_eq!(ignored, ["myapp.second"]);
            assert!(!info.unmatched);
        }

        #[test]
        fn a_matcher_that_never_errors_reports_no_errors() {
            let mut live_checker =
                checker_with(include_str!("../fixtures/cel/span-checkout/matchers.toml"));
            let sample_match = compare_and_record(&mut live_checker, &checkout_span());
            assert!(sample_match.errors.is_empty());
            assert!(recorded(&live_checker).is_none());
        }
    }

    /// A matched span is checked against its span signal.
    mod span_signal {
        use super::*;
        use crate::advice::StabilityAdvisor;
        use crate::{
            sample_span::SampleSpan, CumulativeStatistics, LiveCheckRunner, LiveCheckStatistics,
            Sample,
        };
        use weaver_forge::registry::ResolvedGroup;
        use weaver_semconv::group::{GroupType, SpanKindSpec};

        const MATCHERS: &str = include_str!("../fixtures/cel/span-checkout/matchers.toml");

        /// The findings on the span itself, after a full live check.
        fn check(span: &mut SampleSpan, attributes: Vec<SpanAttribute>) -> Vec<String> {
            let mut live_checker = v2_live_checker_with(attributes, vec![Box::new(TypeAdvisor)]);
            live_checker
                .set_matchers(&matcher_configs(MATCHERS))
                .expect("they check out");
            let mut stats =
                LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
            let parent = Sample::Span(span.clone());
            span.run_live_check(&mut live_checker, &mut stats, None, &parent)
                .expect("the check runs");
            span.live_check_result
                .as_ref()
                .expect("it has a result")
                .all_advice
                .iter()
                .map(|finding| finding.id.clone())
                .collect()
        }

        fn checkout_span() -> SampleSpan {
            serde_json::from_str(include_str!(
                "../fixtures/cel/span-checkout/span-checkout-payment.json"
            ))
            .expect("the fixture sample parses")
        }

        #[test]
        fn a_matched_span_missing_a_recommended_attribute_is_reported() {
            let mut attributes = sample_span_attributes();
            attributes.push(span_attribute(
                "myapp.checkout.coupon",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
            ));
            let ids = check(&mut checkout_span(), attributes);
            assert_eq!(ids, ["recommended_attribute_not_present"]);
        }

        #[test]
        fn a_matched_span_missing_a_required_attribute_is_reported() {
            let mut attributes = sample_span_attributes();
            attributes.push(span_attribute(
                "myapp.checkout.total",
                RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
            ));
            let ids = check(&mut checkout_span(), attributes);
            assert_eq!(ids, ["required_attribute_not_present"]);
        }

        #[test]
        fn a_matched_span_with_every_attribute_is_clean() {
            let ids = check(&mut checkout_span(), sample_span_attributes());
            assert!(ids.is_empty(), "{ids:?}");
        }

        /// The fixture span signal is `internal`.
        #[test]
        fn a_span_kind_that_differs_from_the_signal_is_reported() {
            let mut span = checkout_span();
            span.kind = SpanKindSpec::Client;
            let ids = check(&mut span, sample_span_attributes());
            assert_eq!(ids, ["kind_mismatch"]);
        }

        /// The findings on the span's attributes, after a full live check.
        fn check_attributes(
            catalog: Vec<V2Attribute>,
            attributes: Vec<SpanAttribute>,
        ) -> Vec<String> {
            let mut live_checker = v2_live_checker_full(
                catalog,
                attributes,
                Vec::new(),
                vec![Box::new(StabilityAdvisor), Box::new(TypeAdvisor)],
            );
            live_checker
                .set_matchers(&matcher_configs(MATCHERS))
                .expect("they check out");
            let mut stats =
                LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
            let mut span = checkout_span();
            let parent = Sample::Span(span.clone());
            span.run_live_check(&mut live_checker, &mut stats, None, &parent)
                .expect("the check runs");
            span.attributes
                .iter()
                .filter_map(|attribute| attribute.live_check_result.as_ref())
                .flat_map(|result| result.all_advice.iter())
                .map(|finding| finding.id.clone())
                .collect()
        }

        /// The base definition stays stable, so only the refinement can raise this.
        #[test]
        fn a_refined_stability_is_reported_while_the_base_is_stable() {
            let ids = check_attributes(
                vec![
                    base_attribute("myapp.checkout.id", Stability::Stable),
                    base_attribute("myapp.checkout.stage", Stability::Stable),
                ],
                vec![
                    span_attribute(
                        "myapp.checkout.id",
                        RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                    ),
                    refined_span_attribute("myapp.checkout.stage", Stability::Development),
                ],
            );
            assert_eq!(ids, ["not_stable"]);
        }

        /// The catalog holds it, but only the signal's own copy is compared.
        #[test]
        fn an_attribute_the_signal_does_not_declare_is_missing() {
            let ids = check_attributes(
                vec![
                    base_attribute("myapp.checkout.id", Stability::Stable),
                    base_attribute("myapp.checkout.stage", Stability::Development),
                ],
                vec![span_attribute(
                    "myapp.checkout.id",
                    RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                )],
            );
            assert_eq!(ids, ["missing_attribute"]);
        }

        /// A v1 span group of the fixture registry.
        fn v1_group() -> ResolvedGroup {
            ResolvedGroup {
                id: "myapp.checkout".to_owned(),
                r#type: GroupType::Span,
                brief: String::new(),
                note: String::new(),
                prefix: String::new(),
                entity_associations: Vec::new(),
                extends: None,
                stability: Some(Stability::Stable),
                deprecated: None,
                attributes: Vec::new(),
                span_kind: None,
                events: Vec::new(),
                metric_name: None,
                instrument: None,
                unit: None,
                name: None,
                display_name: None,
                body: None,
                annotations: None,
                lineage: None,
                requirement_level: None,
            }
        }

        #[test]
        fn a_v1_signal_has_no_refined_attributes() {
            let live_checker = v1_live_checker();
            let group = VersionedSignal::Group(Box::new(v1_group()));
            assert!(live_checker
                .find_refined_attribute(&group, "myapp.checkout.stage")
                .is_none());
        }

        const DEPENDENCY_URL: &str = "https://example.com/shared/1.0.0";

        /// The fixture registry, with `dependency` as its one dependency.
        fn v2_live_checker_with_dependency(dependency: ForgeDependency) -> LiveChecker {
            v2_live_checker_with_base_attributes(
                vec![base_attribute("myapp.checkout.id", Stability::Stable)],
                dependency,
            )
        }

        /// The fixture registry declaring `base_attributes`, with `dependency`
        /// as its one dependency.
        fn v2_live_checker_with_base_attributes(
            base_attributes: Vec<V2Attribute>,
            dependency: ForgeDependency,
        ) -> LiveChecker {
            let mut registry: ForgeResolvedRegistry =
                serde_json::from_str(include_str!("../fixtures/registry-v2.json"))
                    .expect("the fixture registry parses");
            if let Some(span) = registry.registry.spans.first_mut() {
                span.attributes = vec![span_attribute(
                    "myapp.checkout.id",
                    RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                )];
            }
            registry.registry.attributes = base_attributes;
            registry.dependencies = BTreeMap::from([(
                DEPENDENCY_URL.try_into().expect("valid schema url"),
                dependency,
            )]);
            LiveChecker::new(
                Arc::new(VersionedRegistry::V2(Box::new(registry))),
                vec![Box::new(TypeAdvisor)],
            )
        }

        /// A registry declaring `myapp.checkout.stage`, which the fixture span
        /// does not.
        fn dependency_registry() -> ForgeDependency {
            let mut registry: ForgeResolvedRegistry =
                serde_json::from_str(include_str!("../fixtures/registry-v2.json"))
                    .expect("the fixture registry parses");
            registry.registry.attributes =
                vec![base_attribute("myapp.checkout.stage", Stability::Stable)];
            ForgeDependency {
                registry: registry.registry,
                refinements: registry.refinements,
            }
        }

        /// The finding ids on the span, after a full live check.
        fn check_findings(live_checker: &mut LiveChecker) -> Vec<String> {
            let mut stats =
                LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
            let mut span = checkout_span();
            let parent = Sample::Span(span.clone());
            span.run_live_check(live_checker, &mut stats, None, &parent)
                .expect("the check runs");
            span.live_check_result
                .as_ref()
                .expect("it has a result")
                .all_advice
                .iter()
                .map(|finding| finding.id.clone())
                .collect()
        }

        /// The findings on the span, with `myapp.common` declaring
        /// `attribute_group_attributes`.
        fn check_with_attribute_group(
            attribute_group_attributes: Vec<AttributeGroupAttribute>,
            matchers: &str,
        ) -> Vec<String> {
            let mut live_checker = v2_live_checker_full(
                Vec::new(),
                vec![span_attribute(
                    "myapp.checkout.id",
                    RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                )],
                attribute_group_attributes,
                vec![Box::new(TypeAdvisor)],
            );
            live_checker
                .set_matchers(&matcher_configs(matchers))
                .expect("they check out");
            let mut stats =
                LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
            let mut span = checkout_span();
            let parent = Sample::Span(span.clone());
            span.run_live_check(&mut live_checker, &mut stats, None, &parent)
                .expect("the check runs");
            span.live_check_result
                .as_ref()
                .expect("it has a result")
                .all_advice
                .iter()
                .map(|finding| finding.id.clone())
                .collect()
        }

        /// Reported once, as `find_attribute` resolves it once.
        #[test]
        fn the_signal_wins_over_a_group_that_declares_the_same_key() {
            let mut live_checker = v2_live_checker_full(
                Vec::new(),
                vec![span_attribute(
                    "myapp.checkout.coupon",
                    RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended),
                )],
                vec![attribute_group_attribute("myapp.checkout.coupon")],
                vec![Box::new(TypeAdvisor)],
            );
            live_checker
                .set_matchers(&matcher_configs(
                    r#"
[[live-check.matchers]]
id = "myapp.checkout"
sample_type = "span"
when = 'name == "checkout payment"'
signal = "myapp.checkout"
attribute_groups = ["myapp.common"]
"#,
                ))
                .expect("they check out");
            let ids = check_findings(&mut live_checker);
            assert_eq!(
                ids.iter()
                    .filter(|id| *id == "recommended_attribute_not_present")
                    .count(),
                1,
                "got: {ids:?}"
            );
        }

        #[test]
        fn the_first_group_wins_over_a_later_one_that_declares_the_same_key() {
            let mut live_checker = two_group_live_checker();
            live_checker
                .set_matchers(&matcher_configs(
                    r#"
[[live-check.matchers]]
id = "myapp.session"
sample_type = "span"
when = 'name == "checkout payment"'
attribute_groups = ["myapp.common"]

[[live-check.matchers]]
id = "myapp.customer"
sample_type = "span"
when = 'name == "checkout payment"'
attribute_groups = ["myapp.extra"]
"#,
                ))
                .expect("they check out");
            let ids = check_findings(&mut live_checker);
            assert_eq!(
                ids.iter()
                    .filter(|id| *id == "recommended_attribute_not_present")
                    .count(),
                1,
                "got: {ids:?}"
            );
        }

        /// The fixture registry, with `myapp.common` and `myapp.extra` both
        /// declaring `myapp.checkout.coupon`, which the span does not carry.
        fn two_group_live_checker() -> LiveChecker {
            let mut registry: ForgeResolvedRegistry =
                serde_json::from_str(include_str!("../fixtures/registry-v2.json"))
                    .expect("the fixture registry parses");
            if let Some(span) = registry.registry.spans.first_mut() {
                span.attributes = Vec::new();
            }
            let mut group = registry
                .registry
                .attribute_groups
                .first()
                .expect("the fixture declares one")
                .clone();
            group.attributes = vec![attribute_group_attribute("myapp.checkout.coupon")];
            let mut extra = group.clone();
            extra.id = "myapp.extra".to_owned().into();
            registry.registry.attribute_groups = vec![group, extra];
            LiveChecker::new(
                Arc::new(VersionedRegistry::V2(Box::new(registry))),
                vec![Box::new(TypeAdvisor)],
            )
        }

        /// The signal declares `myapp.checkout.id` but not `myapp.checkout.stage`.
        #[test]
        fn an_attribute_on_neither_the_signal_nor_a_group_is_unexpected() {
            let ids = check_with_attribute_group(Vec::new(), MATCHERS);
            assert_eq!(ids, ["unexpected_attribute"]);
        }

        #[test]
        fn an_attribute_group_accounts_for_the_attribute() {
            let ids = check_with_attribute_group(
                vec![attribute_group_attribute("myapp.checkout.stage")],
                r#"
[[live-check.matchers]]
id = "myapp.checkout"
sample_type = "span"
when = '"myapp.checkout.id" in attributes'
signal = "myapp.checkout"
attribute_groups = ["myapp.common"]
"#,
            );
            assert!(ids.is_empty(), "{ids:?}");
        }

        /// An attribute only a dependency declares is checked against that
        /// definition, not just named.
        #[test]
        fn a_dependency_definition_is_used_for_the_checks() {
            let mut dependency = dependency_registry();
            dependency.registry.attributes = vec![base_attribute(
                "myapp.checkout.stage",
                Stability::Development,
            )];
            let mut live_checker = v2_live_checker_with_dependency(dependency);
            live_checker.add_advisor(Box::new(StabilityAdvisor));
            live_checker
                .set_matchers(&matcher_configs(MATCHERS))
                .expect("they check out");
            live_checker
                .search_all_attributes()
                .expect("the fixture registry is v2");

            let ids = attribute_findings(&mut live_checker);
            assert_eq!(ids, ["not_stable"]);
        }

        #[test]
        fn without_search_all_attributes_a_dependency_definition_is_not_used() {
            let mut dependency = dependency_registry();
            dependency.registry.attributes = vec![base_attribute(
                "myapp.checkout.stage",
                Stability::Development,
            )];
            let mut live_checker = v2_live_checker_with_dependency(dependency);
            live_checker.add_advisor(Box::new(StabilityAdvisor));
            live_checker
                .set_matchers(&matcher_configs(MATCHERS))
                .expect("they check out");

            let ids = attribute_findings(&mut live_checker);
            assert_eq!(ids, ["missing_attribute"]);
        }

        /// The findings on the span's attributes, after a full live check.
        fn attribute_findings(live_checker: &mut LiveChecker) -> Vec<String> {
            let mut stats =
                LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
            let mut span = checkout_span();
            let parent = Sample::Span(span.clone());
            span.run_live_check(live_checker, &mut stats, None, &parent)
                .expect("the check runs");
            span.attributes
                .iter()
                .filter_map(|attribute| attribute.live_check_result.as_ref())
                .flat_map(|result| result.all_advice.iter())
                .map(|finding| finding.id.clone())
                .collect()
        }

        /// Two registries declaring the same key are both named.
        #[test]
        fn every_schema_that_declares_the_attribute_is_named() {
            let mut dependency = dependency_registry();
            dependency.registry.attributes =
                vec![base_attribute("myapp.checkout.id", Stability::Stable)];
            let mut live_checker = v2_live_checker_with_dependency(dependency);
            live_checker
                .search_all_attributes()
                .expect("the fixture registry is v2");
            let found = live_checker
                .find_base_attribute("myapp.checkout.id")
                .expect("both declare it");
            assert_eq!(
                found.schema_urls(),
                "https://example.com/myapp/1.0.0, https://example.com/shared/1.0.0"
            );
        }

        /// This registry's own definition wins over a dependency's.
        #[test]
        fn a_base_definition_in_this_registry_is_found_too() {
            let mut live_checker = v2_live_checker_with_dependency(dependency_registry());
            live_checker
                .search_all_attributes()
                .expect("the fixture registry is v2");
            let found = live_checker
                .find_base_attribute("myapp.checkout.id")
                .expect("this registry declares it");
            assert_eq!(found.schema_urls(), "https://example.com/myapp/1.0.0");
        }

        #[test]
        fn an_unexpected_attribute_names_the_schema_that_declares_it() {
            let dependency = dependency_registry();
            let mut live_checker = v2_live_checker_with_dependency(dependency);
            live_checker
                .set_matchers(&matcher_configs(MATCHERS))
                .expect("they check out");
            live_checker
                .search_all_attributes()
                .expect("the fixture registry is v2");

            let found = live_checker
                .find_base_attribute("myapp.checkout.stage")
                .expect("the dependency declares it");
            assert_eq!(found.schema_urls(), DEPENDENCY_URL);

            let ids = check_findings(&mut live_checker);
            assert_eq!(ids, ["unexpected_attribute"]);
        }

        /// A key extending a dependency's template resolves to it.
        #[test]
        fn a_dependencys_template_declares_a_key_that_extends_it() {
            let mut dependency = dependency_registry();
            dependency.registry.attributes = vec![template_attribute(
                "myapp.checkout.",
                Stability::Development,
            )];
            let mut live_checker = v2_live_checker_with_dependency(dependency);
            live_checker.add_advisor(Box::new(StabilityAdvisor));
            live_checker
                .search_all_attributes()
                .expect("the fixture registry is v2");
            live_checker
                .set_matchers(&matcher_configs(MATCHERS))
                .expect("they check out");

            let found = live_checker
                .find_base_template("myapp.checkout.stage")
                .expect("the dependency declares the template it extends");
            assert_eq!(found.schema_urls(), DEPENDENCY_URL);
            assert!(!found.declared_here);

            let ids = attribute_findings(&mut live_checker);
            assert_eq!(ids, ["template_attribute", "not_stable"]);
        }

        /// The longest template an extending key matches wins.
        #[test]
        fn the_longest_base_template_that_the_key_extends_wins() {
            let mut dependency = dependency_registry();
            dependency.registry.attributes = vec![
                template_attribute("myapp.", Stability::Stable),
                template_attribute("myapp.checkout.", Stability::Stable),
            ];
            let mut live_checker = v2_live_checker_with_dependency(dependency);
            live_checker
                .search_all_attributes()
                .expect("the fixture registry is v2");
            let found = live_checker
                .find_base_template("myapp.checkout.stage")
                .expect("both templates match the key");
            assert_eq!(found.attribute.name(), "myapp.checkout.");
        }

        /// A key extending this registry's own template is not a dependency's.
        #[test]
        fn a_key_extending_this_registrys_template_is_not_a_dependencys() {
            let mut dependency = dependency_registry();
            dependency.registry.attributes = Vec::new();
            let mut live_checker = v2_live_checker_with_base_attributes(
                vec![template_attribute("myapp.checkout.", Stability::Stable)],
                dependency,
            );
            live_checker
                .search_all_attributes()
                .expect("the fixture registry is v2");

            let found = live_checker
                .find_base_template("myapp.checkout.stage")
                .expect("this registry declares the template it extends");
            assert!(found.declared_here);
            // No matcher, so the span holds no attribute definitions of its
            // own and only a dependency's definition would be reported.
            let ids = check_findings(&mut live_checker);
            assert!(ids.is_empty(), "got: {ids:?}");
        }

        #[test]
        fn without_search_all_attributes_no_base_definition_is_found() {
            let mut live_checker = v2_live_checker_with_dependency(dependency_registry());
            live_checker
                .set_matchers(&matcher_configs(MATCHERS))
                .expect("they check out");
            assert!(live_checker
                .find_base_attribute("myapp.checkout.stage")
                .is_none());
        }

        /// A v1 group carries no attribute list to compare with, so v1 keeps
        /// checking each attribute against the whole registry.
        #[test]
        fn a_v1_signal_raises_no_unexpected_attribute() {
            let live_checker = v1_live_checker();
            let sample_match = SampleMatch {
                signal: Some(Rc::new(VersionedSignal::Group(Box::new(v1_group())))),
                applied: vec![0],
                ..SampleMatch::default()
            };
            let sample = checkout_span();
            let mut result = LiveCheckResult::new();
            let parent = Sample::Span(sample.clone());
            sample_match.add_findings(
                &SampleRef::Span(&sample),
                &sample.attributes,
                &mut result,
                &live_checker,
                &parent,
            );
            assert!(result.all_advice.is_empty());
        }

        /// An unmatched sample has no signal to resolve against, so without
        /// the base definitions nothing declares its attributes.
        #[test]
        fn an_unmatched_span_still_checks_its_attributes() {
            let ids = unmatched_attribute_findings(false);
            assert_eq!(ids, ["missing_attribute", "missing_attribute"]);
        }

        #[test]
        fn search_all_attributes_resolves_an_unmatched_spans_attributes() {
            let ids = unmatched_attribute_findings(true);
            assert_eq!(ids, ["not_stable"]);
        }

        /// The findings on the attributes of a span that matched nothing.
        fn unmatched_attribute_findings(search_all: bool) -> Vec<String> {
            let mut live_checker = v2_live_checker_full(
                vec![
                    base_attribute("myapp.checkout.id", Stability::Stable),
                    base_attribute("myapp.checkout.stage", Stability::Development),
                ],
                sample_span_attributes(),
                Vec::new(),
                vec![Box::new(StabilityAdvisor)],
            );
            live_checker
                .set_matchers(&matcher_configs(
                    r#"
[[live-check.matchers]]
id = "myapp.never"
sample_type = "span"
when = 'name == "no-such-span"'
"#,
                ))
                .expect("they check out");
            if search_all {
                live_checker
                    .search_all_attributes()
                    .expect("the fixture registry is v2");
            }
            let mut stats =
                LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
            let mut span = checkout_span();
            let parent = Sample::Span(span.clone());
            span.run_live_check(&mut live_checker, &mut stats, None, &parent)
                .expect("the check runs");
            span.attributes
                .iter()
                .filter_map(|attribute| attribute.live_check_result.as_ref())
                .flat_map(|result| result.all_advice.iter())
                .map(|finding| finding.id.clone())
                .collect()
        }

        /// An unmatched sample has nothing to be unexpected against.
        #[test]
        fn an_unmatched_span_raises_no_unexpected_attribute() {
            let mut live_checker =
                v2_live_checker_with(sample_span_attributes(), vec![Box::new(TypeAdvisor)]);
            live_checker
                .set_matchers(&matcher_configs(
                    r#"
[[live-check.matchers]]
id = "myapp.never"
sample_type = "span"
when = 'name == "no-such-span"'
"#,
                ))
                .expect("they check out");
            let mut stats =
                LiveCheckStatistics::Cumulative(CumulativeStatistics::new(&live_checker.registry));
            let mut span = checkout_span();
            let parent = Sample::Span(span.clone());
            span.run_live_check(&mut live_checker, &mut stats, None, &parent)
                .expect("the check runs");
            let result = span.live_check_result.as_ref().expect("it has a result");
            let ids: Vec<_> = result
                .all_advice
                .iter()
                .map(|finding| finding.id.clone())
                .collect();
            assert!(ids.is_empty(), "got: {ids:?}");
            assert!(result
                .match_info
                .as_ref()
                .is_some_and(|info| info.unmatched));
        }

        #[test]
        fn a_span_that_matches_nothing_is_not_checked_against_a_signal() {
            let mut span: SampleSpan = serde_json::from_str(include_str!(
                "../fixtures/cel/span-checkout/span-no-signature.json"
            ))
            .expect("the fixture sample parses");
            span.kind = SpanKindSpec::Client;
            let ids = check(
                &mut span,
                vec![span_attribute(
                    "myapp.checkout.total",
                    RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
                )],
            );
            assert!(ids.is_empty(), "got: {ids:?}");
            assert!(
                span.live_check_result
                    .as_ref()
                    .and_then(|result| result.match_info.as_ref())
                    .is_some_and(|info| info.unmatched),
                "the span records that nothing matched it"
            );
        }
    }

    #[test]
    fn every_matcher_sample_type_maps_to_a_sample_type_with_variables() {
        for sample_type in [
            MatcherSampleType::Span,
            MatcherSampleType::SpanEvent,
            MatcherSampleType::SpanLink,
            MatcherSampleType::Log,
            MatcherSampleType::Metric,
            MatcherSampleType::Resource,
            MatcherSampleType::InstrumentationScope,
            MatcherSampleType::Profile,
        ] {
            assert!(
                !variables(sample_type.into()).is_empty(),
                "{sample_type} has no variables"
            );
        }
    }
}
