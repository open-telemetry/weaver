// SPDX-License-Identifier: Apache-2.0

//! Matchers: the CEL expression that gives an untyped sample an identifier.
//!
//! A matcher is declared in `.weaver.toml` as an array of tables under
//! `live-check.matchers`. Its `when` expression is compiled once and then run
//! against every sample of its `sample_type`.

use cel::{Context, Program, Value};
use serde::Deserialize;

use crate::sample::{Sample, SampleType};
use crate::Error;

/// The `live-check` section of the config, holding the matchers.
#[derive(Debug, Deserialize)]
pub struct MatchersConfig {
    /// The matchers, in the order they are declared.
    #[serde(default)]
    pub matchers: Vec<MatcherConfig>,
}

/// One `[[live-check.matchers]]` table, as written in the config.
#[derive(Debug, Deserialize)]
pub struct MatcherConfig {
    /// Names the matcher in findings, statistics and coverage.
    pub id: String,
    /// The kind of sample this matcher looks at.
    pub sample_type: SampleType,
    /// The matcher expression. Absent means every sample of this type matches.
    pub when: Option<String>,
    /// The one signal the sample is compared with.
    pub signal: Option<String>,
    /// Attribute groups, in priority order, added to the comparison.
    #[serde(default)]
    pub attribute_groups: Vec<String>,
}

/// A matcher with its expression compiled.
#[derive(Debug)]
pub struct Matcher {
    config: MatcherConfig,
    program: Option<Program>,
}

impl Matcher {
    /// Compiles the matcher expression, if there is one.
    pub fn compile(config: MatcherConfig) -> Result<Self, Error> {
        let program = config
            .when
            .as_deref()
            .map(|when| {
                Program::compile(when).map_err(|error| Error::CompileFailed {
                    matcher_id: config.id.clone(),
                    error: error.to_string(),
                })
            })
            .transpose()?;
        Ok(Self { config, program })
    }

    /// The matcher config it was compiled from.
    #[must_use]
    pub fn config(&self) -> &MatcherConfig {
        &self.config
    }

    /// Whether this matcher applies to the sample.
    ///
    /// The sample type has to line up and the expression has to come out true.
    /// A matcher with no expression applies to every sample of its type.
    pub fn matches(&self, sample: &Sample) -> Result<bool, Error> {
        if sample.sample_type() != self.config.sample_type {
            return Ok(false);
        }
        let Some(program) = &self.program else {
            return Ok(true);
        };
        let mut context = Context::default();
        sample.bind(&mut context);
        let value = program
            .execute(&context)
            .map_err(|error| Error::EvalFailed {
                matcher_id: self.config.id.clone(),
                error: error.to_string(),
            })?;
        match value {
            Value::Bool(matched) => Ok(matched),
            other => Err(Error::NotBoolean {
                matcher_id: self.config.id.clone(),
                value_type: other.type_of().to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compiles the matchers in a fixture config.
    fn matchers(toml: &str) -> Vec<Matcher> {
        let config: toml::Value = toml::from_str(toml).expect("the fixture config parses");
        let live_check: MatchersConfig = config
            .get("live-check")
            .cloned()
            .expect("the fixture has a live-check section")
            .try_into()
            .expect("the live-check section parses");
        live_check
            .matchers
            .into_iter()
            .map(|matcher| Matcher::compile(matcher).expect("the fixture expressions compile"))
            .collect()
    }

    /// Compiles the one matcher in a fixture config.
    fn matcher(toml: &str) -> Matcher {
        let mut matchers = matchers(toml);
        assert_eq!(matchers.len(), 1, "the fixture declares one matcher");
        matchers.pop().expect("there is one matcher")
    }

    fn sample(json: &str) -> Sample {
        serde_json::from_str(json).expect("the fixture sample parses")
    }

    /// A span with a signature of attributes, standing in for the identifier
    /// it does not have.
    mod span_checkout {
        use super::*;

        fn checkout() -> Matcher {
            matcher(include_str!("../fixtures/span-checkout/matchers.toml"))
        }

        #[test]
        fn config_is_read_from_toml() {
            let matcher = checkout();
            let config = matcher.config();
            assert_eq!(config.id, "myapp.checkout");
            assert_eq!(config.sample_type, SampleType::Span);
            assert_eq!(config.signal.as_deref(), Some("myapp.checkout"));
            assert!(config.attribute_groups.is_empty());
        }

        #[test]
        fn signature_present_and_stage_expected() {
            let sample = sample(include_str!(
                "../fixtures/span-checkout/span-checkout-payment.json"
            ));
            assert!(checkout().matches(&sample).expect("it evaluates"));
        }

        /// The span name looks right, but the name is not part of the
        /// signature.
        #[test]
        fn signature_attribute_missing() {
            let sample = sample(include_str!(
                "../fixtures/span-checkout/span-no-signature.json"
            ));
            assert!(!checkout().matches(&sample).expect("it evaluates"));
        }

        #[test]
        fn stage_does_not_match_the_regex() {
            let sample = sample(include_str!(
                "../fixtures/span-checkout/span-checkout-refund.json"
            ));
            assert!(!checkout().matches(&sample).expect("it evaluates"));
        }

        /// A matcher never sees a sample of another type, whatever its
        /// expression says.
        #[test]
        fn a_log_is_not_offered_to_a_span_matcher() {
            let sample = sample(include_str!("../fixtures/log-common/log-order-placed.json"));
            assert!(!checkout().matches(&sample).expect("it evaluates"));
        }
    }

    /// A group added to every log, since logs have no signal of their own.
    mod log_common {
        use super::*;

        fn common() -> Matcher {
            matcher(include_str!("../fixtures/log-common/matchers.toml"))
        }

        #[test]
        fn config_adds_a_group_and_no_signal() {
            let matcher = common();
            let config = matcher.config();
            assert_eq!(config.sample_type, SampleType::Log);
            assert!(config.when.is_none());
            assert!(config.signal.is_none());
            assert_eq!(config.attribute_groups, ["myapp.common"]);
        }

        /// No `when`, so every log of this type matches.
        #[test]
        fn a_log_that_names_an_event_matches() {
            let sample = sample(include_str!("../fixtures/log-common/log-order-placed.json"));
            assert!(common().matches(&sample).expect("it evaluates"));
        }

        /// The event match is what we lose without an identifier. The group is
        /// still added.
        #[test]
        fn a_log_with_no_event_name_matches_too() {
            let sample = sample(include_str!(
                "../fixtures/log-common/log-no-event-name.json"
            ));
            assert!(common().matches(&sample).expect("it evaluates"));
        }

        /// The log fields are readable, and an absent `event_name` is empty
        /// rather than unbound.
        #[test]
        fn log_fields_are_bound() {
            let named = sample(include_str!("../fixtures/log-common/log-order-placed.json"));
            let unnamed = sample(include_str!(
                "../fixtures/log-common/log-no-event-name.json"
            ));
            let matcher = Matcher::compile(MatcherConfig {
                id: "log.fields".to_owned(),
                sample_type: SampleType::Log,
                when: Some(
                    r#"event_name == "myapp.order.placed" && severity_number == 9"#.to_owned(),
                ),
                signal: None,
                attribute_groups: Vec::new(),
            })
            .expect("it compiles");
            assert!(matcher.matches(&named).expect("it evaluates"));
            assert!(!matcher.matches(&unnamed).expect("it evaluates"));
        }
    }

    /// The checkout matcher with a group on top, so the extra attribute is
    /// expected rather than unexpected.
    mod span_checkout_additional_attributes {
        use super::*;

        #[test]
        fn the_signature_is_unchanged_and_the_group_is_added() {
            let matcher = matcher(include_str!(
                "../fixtures/span-checkout-additional-attributes/matchers.toml"
            ));
            assert_eq!(matcher.config().signal.as_deref(), Some("myapp.checkout"));
            assert_eq!(matcher.config().attribute_groups, ["myapp.common"]);
            let sample = sample(include_str!(
                "../fixtures/span-checkout-additional-attributes/span-checkout-payment-tenant.json"
            ));
            assert!(matcher.matches(&sample).expect("it evaluates"));
        }
    }

    /// A group added to our own metrics, each of which keeps its natural
    /// match.
    mod metric_common {
        use super::*;

        fn common() -> Matcher {
            matcher(include_str!("../fixtures/metric-common/matchers.toml"))
        }

        #[test]
        fn config_has_no_signal_so_the_natural_match_stands() {
            let matcher = common();
            assert_eq!(matcher.config().sample_type, SampleType::Metric);
            assert!(matcher.config().signal.is_none());
        }

        #[test]
        fn our_own_metric_matches_on_its_name() {
            let sample = sample(include_str!(
                "../fixtures/metric-common/metric-myapp-checkout-duration.json"
            ));
            assert!(common().matches(&sample).expect("it evaluates"));
        }

        /// The metrics from the libraries we depend on are left as they were.
        #[test]
        fn a_library_metric_is_left_alone() {
            let sample = sample(include_str!(
                "../fixtures/metric-common/metric-http-client-request-duration.json"
            ));
            assert!(!common().matches(&sample).expect("it evaluates"));
        }

        /// On a metric, `attributes` is all of the data point attributes
        /// together.
        #[test]
        fn attributes_are_the_data_points_together() {
            let matcher = Matcher::compile(MatcherConfig {
                id: "metric.attributes".to_owned(),
                sample_type: SampleType::Metric,
                when: Some(
                    r#""myapp.checkout.stage" in attributes && "myapp.tenant.code" in attributes"#
                        .to_owned(),
                ),
                signal: None,
                attribute_groups: Vec::new(),
            })
            .expect("it compiles");
            let sample = sample(include_str!(
                "../fixtures/metric-common/metric-myapp-checkout-duration.json"
            ));
            assert!(matcher.matches(&sample).expect("it evaluates"));
        }
    }

    /// A resource is a list of attributes, so the only thing to test is those
    /// attributes.
    mod resource {
        use super::*;

        fn ours() -> Matcher {
            matcher(include_str!("../fixtures/resource/matchers.toml"))
        }

        #[test]
        fn config_has_no_signal() {
            let matcher = ours();
            assert_eq!(matcher.config().sample_type, SampleType::Resource);
            assert!(matcher.config().signal.is_none());
            assert_eq!(matcher.config().attribute_groups, ["myapp.resource"]);
        }

        #[test]
        fn our_own_service_matches() {
            let sample = sample(include_str!(
                "../fixtures/resource/resource-myapp-checkout.json"
            ));
            assert!(ours().matches(&sample).expect("it evaluates"));
        }

        #[test]
        fn another_service_does_not() {
            let sample = sample(include_str!(
                "../fixtures/resource/resource-other-service.json"
            ));
            assert!(!ours().matches(&sample).expect("it evaluates"));
        }

        /// The `in` test is what keeps this from erroring.
        #[test]
        fn a_resource_without_a_service_name_does_not_error() {
            let sample = sample(include_str!(
                "../fixtures/resource/resource-no-service-name.json"
            ));
            assert!(!ours().matches(&sample).expect("it evaluates"));
        }
    }

    /// A scope has an identifier, so its matcher needs no signature.
    mod instrumentation_scope {
        use super::*;

        fn ours() -> Matcher {
            matcher(include_str!(
                "../fixtures/instrumentation-scope/matchers.toml"
            ))
        }

        #[test]
        fn our_own_instrumentation_matches_on_its_name() {
            let sample = sample(include_str!(
                "../fixtures/instrumentation-scope/scope-myapp-checkout.json"
            ));
            assert!(ours().matches(&sample).expect("it evaluates"));
        }

        #[test]
        fn a_library_scope_does_not() {
            let sample = sample(include_str!(
                "../fixtures/instrumentation-scope/scope-jdbc.json"
            ));
            assert!(!ours().matches(&sample).expect("it evaluates"));
        }

        /// The version and the schema url are readable too.
        #[test]
        fn version_and_schema_url_are_bound() {
            let matcher = Matcher::compile(MatcherConfig {
                id: "scope.fields".to_owned(),
                sample_type: SampleType::InstrumentationScope,
                when: Some(
                    r#"version == "0.3.1" && schema_url.endsWith("/myschema/1.0.0")"#.to_owned(),
                ),
                signal: None,
                attribute_groups: Vec::new(),
            })
            .expect("it compiles");
            let sample = sample(include_str!(
                "../fixtures/instrumentation-scope/scope-myapp-checkout.json"
            ));
            assert!(matcher.matches(&sample).expect("it evaluates"));
        }
    }

    /// The signature is only trusted when the span came from our own
    /// instrumentation.
    mod span_checkout_scoped {
        use super::*;

        fn checkout() -> Matcher {
            matcher(include_str!(
                "../fixtures/span-checkout-scoped/matchers.toml"
            ))
        }

        #[test]
        fn a_span_from_our_own_instrumentation_matches() {
            let sample = sample(include_str!(
                "../fixtures/span-checkout-scoped/span-myapp-scope.json"
            ));
            assert!(checkout().matches(&sample).expect("it evaluates"));
        }

        /// The same attributes from somewhere else no longer match.
        #[test]
        fn the_same_span_from_another_library_does_not() {
            let sample = sample(include_str!(
                "../fixtures/span-checkout-scoped/span-other-scope.json"
            ));
            assert!(!checkout().matches(&sample).expect("it evaluates"));
        }

        /// The resource the sample arrived with is readable as well.
        #[test]
        fn the_resource_of_a_signal_sample_is_bound() {
            let matcher = Matcher::compile(MatcherConfig {
                id: "span.resource".to_owned(),
                sample_type: SampleType::Span,
                when: Some(
                    r#""service.name" in resource.attributes
                       && resource.attributes["service.name"] == "myapp.checkout""#
                        .to_owned(),
                ),
                signal: None,
                attribute_groups: Vec::new(),
            })
            .expect("it compiles");
            let sample = sample(include_str!(
                "../fixtures/span-checkout-scoped/span-myapp-scope.json"
            ));
            assert!(matcher.matches(&sample).expect("it evaluates"));
        }

        /// A span that arrived with no scope at all errors rather than coming
        /// out false, because the variable is not there to read.
        #[test]
        fn a_span_with_no_scope_errors() {
            let sample = sample(include_str!(
                "../fixtures/span-checkout/span-checkout-payment.json"
            ));
            let error = checkout().matches(&sample).expect_err("it errors");
            assert!(matches!(error, Error::EvalFailed { .. }), "{error}");
        }
    }

    /// The outcome of a span. OTLP has it, and it is what the `error.type`
    /// conditions in semconv are about.
    mod span_status {
        use super::*;

        fn failed() -> Matcher {
            matcher(include_str!("../fixtures/span-status/matchers.toml"))
        }

        #[test]
        fn a_failed_span_matches() {
            let sample = sample(include_str!("../fixtures/span-status/span-error.json"));
            assert!(failed().matches(&sample).expect("it evaluates"));
        }

        #[test]
        fn a_successful_span_does_not() {
            let sample = sample(include_str!("../fixtures/span-status/span-ok.json"));
            assert!(!failed().matches(&sample).expect("it evaluates"));
        }

        /// A span with no status is `unset`, as in OTLP, so the expression is
        /// false rather than an error.
        #[test]
        fn a_span_with_no_status_is_unset() {
            let sample = sample(include_str!("../fixtures/span-status/span-no-status.json"));
            assert!(!failed().matches(&sample).expect("it evaluates"));
        }

        #[test]
        fn the_status_message_is_readable() {
            let matcher = Matcher::compile(MatcherConfig {
                id: "status.message".to_owned(),
                sample_type: SampleType::Span,
                when: Some(r#"status.message.contains("declined")"#.to_owned()),
                signal: None,
                attribute_groups: Vec::new(),
            })
            .expect("it compiles");
            let sample = sample(include_str!("../fixtures/span-status/span-error.json"));
            assert!(matcher.matches(&sample).expect("it evaluates"));
        }

        /// `error.type` is `conditionally_required: if and only if an error
        /// has occurred.` With the status in the expression the condition is
        /// one clause, and the attribute that goes missing under it is
        /// visible.
        #[test]
        fn the_error_type_condition_can_be_expressed() {
            let condition = Matcher::compile(MatcherConfig {
                id: "error.type.condition".to_owned(),
                sample_type: SampleType::Span,
                when: Some(r#"status.code == "error""#.to_owned()),
                signal: None,
                attribute_groups: Vec::new(),
            })
            .expect("it compiles");
            let missing = Matcher::compile(MatcherConfig {
                id: "error.type.missing".to_owned(),
                sample_type: SampleType::Span,
                when: Some(r#"status.code == "error" && !("error.type" in attributes)"#.to_owned()),
                signal: None,
                attribute_groups: Vec::new(),
            })
            .expect("it compiles");

            let with_error_type = sample(include_str!("../fixtures/span-status/span-error.json"));
            let without = sample(include_str!(
                "../fixtures/span-status/span-error-no-error-type.json"
            ));
            let ok = sample(include_str!("../fixtures/span-status/span-ok.json"));

            // The condition is true on both failed spans, and false on the
            // one that succeeded.
            assert!(condition.matches(&with_error_type).expect("it evaluates"));
            assert!(condition.matches(&without).expect("it evaluates"));
            assert!(!condition.matches(&ok).expect("it evaluates"));

            // Only the failed span without `error.type` is in breach of it.
            assert!(!missing.matches(&with_error_type).expect("it evaluates"));
            assert!(missing.matches(&without).expect("it evaluates"));
            assert!(!missing.matches(&ok).expect("it evaluates"));
        }
    }

    /// The behaviour of CEL itself that the plan relies on.
    mod cel_behaviour {
        use super::*;

        fn span_matcher(when: &str) -> Matcher {
            Matcher::compile(MatcherConfig {
                id: "cel.behaviour".to_owned(),
                sample_type: SampleType::Span,
                when: Some(when.to_owned()),
                signal: None,
                attribute_groups: Vec::new(),
            })
            .expect("it compiles")
        }

        fn payment() -> Sample {
            sample(include_str!(
                "../fixtures/span-checkout/span-checkout-payment.json"
            ))
        }

        /// An unguarded read of an attribute the sample does not carry errors.
        #[test]
        fn an_unguarded_read_errors() {
            let error = span_matcher(r#"attributes["absent"] == "x""#)
                .matches(&payment())
                .expect_err("it errors");
            assert!(matches!(error, Error::EvalFailed { .. }), "{error}");
        }

        /// Error absorption in `&&` is commutative, so the guard works from
        /// either side.
        #[test]
        fn the_guard_absorbs_the_error_from_either_side() {
            let guard_first = r#""absent" in attributes && attributes["absent"] == "x""#;
            let guard_second = r#"attributes["absent"] == "x" && "absent" in attributes"#;
            for when in [guard_first, guard_second] {
                assert!(
                    !span_matcher(when)
                        .matches(&payment())
                        .expect("it evaluates"),
                    "{when}"
                );
            }
        }

        /// The error is only absorbed while the other side is false, so a
        /// guard on the wrong key does not protect the read.
        #[test]
        fn a_guard_on_the_wrong_key_still_errors() {
            let error =
                span_matcher(r#""myapp.checkout.id" in attributes && attributes["absent"] == "x""#)
                    .matches(&payment())
                    .expect_err("it errors");
            assert!(matches!(error, Error::EvalFailed { .. }), "{error}");
        }

        /// A matcher with no `when` applies to every sample of its type.
        #[test]
        fn no_expression_matches_every_sample_of_the_type() {
            let matcher = Matcher::compile(MatcherConfig {
                id: "every.span".to_owned(),
                sample_type: SampleType::Span,
                when: None,
                signal: None,
                attribute_groups: Vec::new(),
            })
            .expect("a matcher with no expression compiles");
            assert!(matcher.matches(&payment()).expect("it evaluates"));
        }

        #[test]
        fn an_expression_that_does_not_parse_is_rejected() {
            let error = Matcher::compile(MatcherConfig {
                id: "broken".to_owned(),
                sample_type: SampleType::Span,
                when: Some("attributes[".to_owned()),
                signal: None,
                attribute_groups: Vec::new(),
            })
            .expect_err("the expression does not compile");
            assert!(matches!(error, Error::CompileFailed { .. }), "{error}");
        }

        /// An expression that is not a bool is a config fault, not a match.
        #[test]
        fn an_expression_that_is_not_a_bool_is_rejected() {
            let error = span_matcher(r#"attributes["myapp.checkout.id"]"#)
                .matches(&payment())
                .expect_err("it is not a bool");
            assert!(matches!(error, Error::NotBoolean { .. }), "{error}");
        }
    }
}
