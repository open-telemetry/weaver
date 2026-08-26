// SPDX-License-Identifier: Apache-2.0

//! Matchers from the live-check config, compiled and checked at startup.

use weaver_cel::Expression;
use weaver_config::live_check::{MatcherConfig, MatcherSampleType};

use crate::{cel::variables, live_checker::LiveChecker, Error, SampleType, VersionedRegistry};

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
}

/// The configured matchers, in declaration order.
#[derive(Debug, Default)]
pub struct Matchers {
    matchers: Vec<Matcher>,
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
            });
        }
        Ok(Self { matchers })
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
            for group in &matcher.attribute_groups {
                if live_checker.find_attribute_group(group).is_none() {
                    return Err(Error::UnknownMatcherAttributeGroup {
                        id: matcher.id.clone(),
                        attribute_group: group.clone(),
                    });
                }
            }
        }
        Ok(())
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

    use weaver_forge::registry::ResolvedRegistry;
    use weaver_forge::v2::registry::ForgeResolvedRegistry;

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
        let registry: ForgeResolvedRegistry =
            serde_json::from_str(include_str!("../fixtures/registry-v2.json"))
                .expect("the fixture registry parses");
        LiveChecker::new(
            Arc::new(VersionedRegistry::V2(Box::new(registry))),
            Vec::new(),
        )
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

    /// A v1 registry has neither, so a v2-only lookup is always `None`.
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
        .expect_err("the group is not there");
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
