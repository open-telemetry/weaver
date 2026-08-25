// SPDX-License-Identifier: Apache-2.0

//! Matchers from the live-check config, compiled and checked at startup.

use weaver_cel::Expression;
use weaver_config::live_check::{MatcherConfig, MatcherSampleType};

use crate::{cel::variables, Error, SampleType};

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
    use super::{fixture::matcher_configs, *};

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
