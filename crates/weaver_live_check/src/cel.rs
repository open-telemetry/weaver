// SPDX-License-Identifier: Apache-2.0

//! Binds live-check samples to CEL variables.
//!
//! Each sample type has its own set of variables. They are added to a
//! [`Context`] through the `cel` crate's serde support, so a field is bound
//! in its serde form: an enum as its variant name, an `Option` as the value
//! or `null`.

use std::collections::HashMap;

use cel::{Context, ExecutionError, Program, SerializationError, Value};
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::{
    sample_attribute::SampleAttribute,
    sample_instrumentation_scope::SampleInstrumentationScope,
    sample_log::SampleLog,
    sample_metric::{DataPoints, SampleMetric},
    sample_profile::SampleProfile,
    sample_resource::SampleResource,
    sample_span::{SampleSpan, SampleSpanEvent, SampleSpanLink},
    SampleType,
};

/// A sample the matchers can select.
pub trait Matchable {
    /// The kind of sample, which decides which matchers apply to it.
    fn sample_type(&self) -> SampleType;

    /// Adds the sample's variables to a CEL context.
    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError>;
}

/// Runs a compiled `when` against a context.
///
/// # Errors
///
/// Returns the interpreter's error, or an `UnexpectedType` error when the
/// expression returns a value that is not a bool.
pub(crate) fn execute(program: &Program, context: &Context<'_>) -> Result<bool, ExecutionError> {
    match program.execute(context)? {
        Value::Bool(result) => Ok(result),
        other => Err(ExecutionError::UnexpectedType {
            got: other.type_of().to_string(),
            want: "bool".to_owned(),
        }),
    }
}

/// The map behind `attributes["key"]`.
type AttributeMap<'a> = HashMap<&'a str, &'a Option<JsonValue>>;

/// The `resource` variable.
#[derive(Serialize)]
struct ResourceVariable<'a> {
    attributes: AttributeMap<'a>,
}

/// The `instrumentation_scope` variable.
#[derive(Serialize)]
struct ScopeVariable<'a> {
    name: &'a str,
    version: &'a str,
    schema_url: &'a str,
    attributes: AttributeMap<'a>,
}

fn attribute_map<'a>(attributes: impl Iterator<Item = &'a SampleAttribute>) -> AttributeMap<'a> {
    attributes
        .map(|attribute| (attribute.name.as_str(), &attribute.value))
        .collect()
}

/// The attributes every data point of a metric agrees on. A key the points
/// hold different values for is left out.
fn agreed_attributes(metric: &SampleMetric) -> AttributeMap<'_> {
    let attributes: Box<dyn Iterator<Item = &SampleAttribute>> = match &metric.data_points {
        Some(DataPoints::Number(points)) => {
            Box::new(points.iter().flat_map(|point| point.attributes.iter()))
        }
        Some(DataPoints::Histogram(points)) => {
            Box::new(points.iter().flat_map(|point| point.attributes.iter()))
        }
        Some(DataPoints::ExponentialHistogram(points)) => {
            Box::new(points.iter().flat_map(|point| point.attributes.iter()))
        }
        None => Box::new(std::iter::empty()),
    };
    let mut agreed = AttributeMap::new();
    let mut disputed = Vec::new();
    for attribute in attributes {
        if let Some(held) = agreed.insert(attribute.name.as_str(), &attribute.value) {
            if *held != attribute.value {
                disputed.push(attribute.name.as_str());
            }
        }
    }
    for name in disputed {
        let _ = agreed.remove(name);
    }
    agreed
}

/// Binds `resource` and `instrumentation_scope`. Either is `null` when the
/// sample did not arrive with it.
fn bind_signal_context(
    resource: Option<&SampleResource>,
    scope: Option<&SampleInstrumentationScope>,
    context: &mut Context<'_>,
) -> Result<(), SerializationError> {
    context.add_variable(
        "resource",
        resource.map(|resource| ResourceVariable {
            attributes: attribute_map(resource.attributes.iter()),
        }),
    )?;
    context.add_variable(
        "instrumentation_scope",
        scope.map(|scope| ScopeVariable {
            name: &scope.name,
            version: &scope.version,
            schema_url: &scope.schema_url,
            attributes: attribute_map(scope.attributes.iter()),
        }),
    )
}

impl Matchable for SampleSpan {
    fn sample_type(&self) -> SampleType {
        SampleType::Span
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("name", &self.name)?;
        context.add_variable("kind", &self.kind)?;
        // OTLP treats a missing status as unset.
        context.add_variable("status", self.status.clone().unwrap_or_default())?;
        context.add_variable("attributes", attribute_map(self.attributes.iter()))?;
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            context,
        )
    }
}

impl Matchable for SampleSpanEvent {
    fn sample_type(&self) -> SampleType {
        SampleType::SpanEvent
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("name", &self.name)?;
        context.add_variable("attributes", attribute_map(self.attributes.iter()))?;
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            context,
        )
    }
}

impl Matchable for SampleSpanLink {
    fn sample_type(&self) -> SampleType {
        SampleType::SpanLink
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("attributes", attribute_map(self.attributes.iter()))?;
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            context,
        )
    }
}

impl Matchable for SampleLog {
    fn sample_type(&self) -> SampleType {
        SampleType::Log
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("event_name", &self.event_name)?;
        context.add_variable("severity_text", &self.severity_text)?;
        context.add_variable("severity_number", self.severity_number)?;
        context.add_variable("body", &self.body)?;
        context.add_variable("attributes", attribute_map(self.attributes.iter()))?;
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            context,
        )
    }
}

impl Matchable for SampleMetric {
    fn sample_type(&self) -> SampleType {
        SampleType::Metric
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("name", &self.name)?;
        context.add_variable("unit", &self.unit)?;
        context.add_variable("instrument", &self.instrument)?;
        context.add_variable("attributes", agreed_attributes(self))?;
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            context,
        )
    }
}

impl Matchable for SampleResource {
    fn sample_type(&self) -> SampleType {
        SampleType::Resource
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("attributes", attribute_map(self.attributes.iter()))
    }
}

impl Matchable for SampleInstrumentationScope {
    fn sample_type(&self) -> SampleType {
        SampleType::InstrumentationScope
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("name", &self.name)?;
        context.add_variable("version", &self.version)?;
        context.add_variable("schema_url", &self.schema_url)?;
        context.add_variable("attributes", attribute_map(self.attributes.iter()))
    }
}

impl Matchable for SampleProfile {
    fn sample_type(&self) -> SampleType {
        SampleType::Profile
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("attributes", attribute_map(self.attributes.iter()))?;
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            context,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::matcher::{fixture, Matchers};

    /// Compiles a fixture that declares one matcher.
    fn compile(toml_str: &str) -> Matchers {
        let configs = fixture::matcher_configs(toml_str);
        assert_eq!(configs.len(), 1, "the fixture declares one matcher");
        Matchers::compile(&configs, &fixture::v2_live_checker())
            .expect("the fixture matchers compile")
    }

    /// Binds a sample and runs a compiled expression against it.
    fn run(program: &Program, sample: &dyn Matchable) -> Result<bool, ExecutionError> {
        let mut context = Context::default();
        sample.bind(&mut context).expect("the sample binds");
        execute(program, &context)
    }

    /// Evaluates the `when` of a fixture against a sample.
    fn matches(toml_str: &str, sample: &dyn Matchable) -> Result<bool, ExecutionError> {
        let matchers = compile(toml_str);
        let matcher = matchers.iter().next().expect("there is one matcher");
        // A fixture with no `when` matches every sample of the type.
        matcher
            .when
            .as_ref()
            .map_or(Ok(true), |when| run(when, sample))
    }

    fn parse<T: serde::de::DeserializeOwned>(json: &str) -> T {
        serde_json::from_str(json).expect("the fixture sample parses")
    }

    fn evaluate(when: &str, sample: &dyn Matchable) -> Result<bool, ExecutionError> {
        run(&Program::compile(when).expect("it compiles"), sample)
    }

    /// A span matched on a signature of attributes.
    mod span_checkout {
        use super::*;

        const MATCHERS: &str = include_str!("../fixtures/cel/span-checkout/matchers.toml");

        fn payment_span() -> SampleSpan {
            parse(include_str!(
                "../fixtures/cel/span-checkout/span-checkout-payment.json"
            ))
        }

        #[test]
        fn config_is_read_from_toml() {
            let matchers = compile(MATCHERS);
            let matcher = matchers.iter().next().expect("there is one matcher");
            assert_eq!(matcher.id, "myapp.checkout");
            assert_eq!(matcher.sample_type, SampleType::Span);
            assert_eq!(matcher.signal.as_deref(), Some("myapp.checkout"));
            assert!(matcher.attribute_groups.is_empty());
            assert!(matcher
                .when
                .as_ref()
                .expect("it has a when")
                .references()
                .has_variable("attributes"));
        }

        #[test]
        fn signature_present_and_stage_expected() {
            assert!(matches(MATCHERS, &payment_span()).expect("it evaluates"));
        }

        /// The name is not part of the signature.
        #[test]
        fn signature_attribute_missing() {
            let span: SampleSpan = parse(include_str!(
                "../fixtures/cel/span-checkout/span-no-signature.json"
            ));
            assert!(!matches(MATCHERS, &span).expect("it evaluates"));
        }

        #[test]
        fn stage_does_not_match_the_regex() {
            let span: SampleSpan = parse(include_str!(
                "../fixtures/cel/span-checkout/span-checkout-refund.json"
            ));
            assert!(!matches(MATCHERS, &span).expect("it evaluates"));
        }

        #[test]
        fn the_span_kind_is_the_serde_name() {
            assert!(evaluate(r#"kind == "internal""#, &payment_span()).expect("it evaluates"));
        }

        #[test]
        fn an_expression_that_is_not_a_bool_is_rejected() {
            let error = evaluate(r#"attributes["myapp.checkout.stage"]"#, &payment_span())
                .expect_err("not a bool");
            assert!(
                matches!(error, ExecutionError::UnexpectedType { .. }),
                "{error}"
            );
        }

        /// A variable the sample type does not have errors on every sample.
        #[test]
        fn an_unbound_variable_errors() {
            let error = evaluate(r#"unit == "s""#, &payment_span()).expect_err("it errors");
            assert!(
                matches!(error, ExecutionError::UndeclaredReference(_)),
                "{error}"
            );
        }

        /// The guard works on either side of the `&&`.
        #[test]
        fn a_guarded_read_of_an_absent_key_is_false_from_either_side() {
            let span = payment_span();
            for when in [
                r#""absent" in attributes && attributes["absent"] == "x""#,
                r#"attributes["absent"] == "x" && "absent" in attributes"#,
            ] {
                assert!(!evaluate(when, &span).expect("it evaluates"), "{when}");
            }
        }
    }

    /// The outcome of a span, from its OTLP status.
    mod span_status {
        use super::*;

        const MATCHERS: &str = include_str!("../fixtures/cel/span-status/matchers.toml");

        fn span(json: &str) -> SampleSpan {
            parse(json)
        }

        #[test]
        fn a_failed_span_matches() {
            let sample = span(include_str!("../fixtures/cel/span-status/span-error.json"));
            assert!(matches(MATCHERS, &sample).expect("it evaluates"));
        }

        #[test]
        fn a_successful_span_does_not() {
            let sample = span(include_str!("../fixtures/cel/span-status/span-ok.json"));
            assert!(!matches(MATCHERS, &sample).expect("it evaluates"));
        }

        #[test]
        fn a_span_with_no_status_is_unset() {
            let sample = span(include_str!(
                "../fixtures/cel/span-status/span-no-status.json"
            ));
            assert!(!matches(MATCHERS, &sample).expect("it evaluates"));
            assert!(evaluate(r#"status.code == "unset""#, &sample).expect("it evaluates"));
        }

        #[test]
        fn the_status_message_is_readable() {
            let sample = span(include_str!("../fixtures/cel/span-status/span-error.json"));
            assert!(
                evaluate(r#"status.message.contains("declined")"#, &sample).expect("it evaluates")
            );
        }

        /// A `when` of `"error.type" in attributes` is circular. It is true only
        /// when the attribute is present, so the condition must use the status.
        #[test]
        fn the_error_type_condition_can_be_expressed() {
            let condition = r#"status.code == "error""#;
            let in_breach = r#"status.code == "error" && !("error.type" in attributes)"#;

            let with_error_type = span(include_str!("../fixtures/cel/span-status/span-error.json"));
            let without = span(include_str!(
                "../fixtures/cel/span-status/span-error-no-error-type.json"
            ));
            let ok = span(include_str!("../fixtures/cel/span-status/span-ok.json"));

            assert!(evaluate(condition, &with_error_type).expect("it evaluates"));
            assert!(evaluate(condition, &without).expect("it evaluates"));
            assert!(!evaluate(condition, &ok).expect("it evaluates"));

            assert!(!evaluate(in_breach, &with_error_type).expect("it evaluates"));
            assert!(evaluate(in_breach, &without).expect("it evaluates"));
            assert!(!evaluate(in_breach, &ok).expect("it evaluates"));
        }
    }

    /// A matcher that only adds an attribute group.
    mod log_common {
        use super::*;

        const MATCHERS: &str = include_str!("../fixtures/cel/log-common/matchers.toml");

        fn log_without_optional_fields() -> SampleLog {
            parse(
                r#"{ "event_name": "myapp.order.placed", "attributes": [], "live_check_result": null }"#,
            )
        }

        #[test]
        fn config_adds_a_group_and_no_signal() {
            let matchers = compile(MATCHERS);
            let matcher = matchers.iter().next().expect("there is one matcher");
            assert_eq!(matcher.sample_type, SampleType::Log);
            assert!(matcher.when.is_none());
            assert!(matcher.signal.is_none());
            assert_eq!(matcher.attribute_groups, ["myapp.common"]);
        }

        #[test]
        fn an_unguarded_read_of_an_absent_optional_field_errors() {
            let log = log_without_optional_fields();
            for when in ["severity_number < 10", r#"body.contains("x")"#] {
                let _ = evaluate(when, &log).expect_err("it errors");
            }
        }

        #[test]
        fn an_absent_optional_field_is_null_and_can_be_guarded() {
            let log = log_without_optional_fields();
            for when in [
                r#"body != null && body.contains("order")"#,
                "severity_number != null && severity_number < 10",
                r#"severity_text != null && severity_text == "INFO""#,
            ] {
                assert!(!evaluate(when, &log).expect("it evaluates"), "{when}");
            }
        }

        #[test]
        fn a_guarded_read_of_a_present_optional_field_matches() {
            let log: SampleLog = parse(include_str!(
                "../fixtures/cel/log-common/log-order-placed.json"
            ));
            for when in [
                r#"body != null && body.contains("order")"#,
                "severity_number != null && severity_number < 10",
                r#"severity_text != null && severity_text == "INFO""#,
            ] {
                assert!(evaluate(when, &log).expect("it evaluates"), "{when}");
            }
        }

        #[test]
        fn an_absent_optional_field_compares_false_rather_than_erroring() {
            assert!(
                !evaluate(r#"severity_text == "INFO""#, &log_without_optional_fields())
                    .expect("it evaluates")
            );
        }

        /// The fixture has no `when`.
        #[test]
        fn every_log_matches() {
            for json in [
                include_str!("../fixtures/cel/log-common/log-order-placed.json"),
                include_str!("../fixtures/cel/log-common/log-no-event-name.json"),
            ] {
                let log: SampleLog = parse(json);
                assert!(matches(MATCHERS, &log).expect("it evaluates"));
            }
        }

        #[test]
        fn log_fields_are_bound() {
            let named: SampleLog = parse(include_str!(
                "../fixtures/cel/log-common/log-order-placed.json"
            ));
            let unnamed: SampleLog = parse(include_str!(
                "../fixtures/cel/log-common/log-no-event-name.json"
            ));
            let when = r#"event_name == "myapp.order.placed" && severity_number == 9"#;
            assert!(evaluate(when, &named).expect("it evaluates"));
            assert!(!evaluate(when, &unnamed).expect("it evaluates"));
        }
    }

    /// A matcher that adds a group to our own metrics.
    mod metric_common {
        use super::*;

        const MATCHERS: &str = include_str!("../fixtures/cel/metric-common/matchers.toml");

        fn ours() -> SampleMetric {
            parse(include_str!(
                "../fixtures/cel/metric-common/metric-myapp-checkout-duration.json"
            ))
        }

        #[test]
        fn a_key_the_data_points_disagree_on_is_left_out() {
            let metric: SampleMetric = parse(
                r#"{
                  "name": "myapp.checkout.duration",
                  "instrument": "histogram",
                  "unit": "s",
                  "data_points": [
                    { "attributes": [{ "name": "myapp.checkout.stage", "value": "payment" },
                                     { "name": "myapp.tenant.code", "value": "acme-eu" }],
                      "value": 0.42 },
                    { "attributes": [{ "name": "myapp.checkout.stage", "value": "cart" },
                                     { "name": "myapp.tenant.code", "value": "acme-eu" }],
                      "value": 1.13 }
                  ],
                  "live_check_result": null
                }"#,
            );
            assert!(
                evaluate(r#"attributes["myapp.tenant.code"] == "acme-eu""#, &metric)
                    .expect("it evaluates"),
                "the points agree on the tenant"
            );
            assert!(
                !evaluate(r#""myapp.checkout.stage" in attributes"#, &metric)
                    .expect("it evaluates"),
                "the points disagree on the stage"
            );
            let error = evaluate(
                r#"attributes["myapp.checkout.stage"] == "payment""#,
                &metric,
            )
            .expect_err("reading it errors");
            assert!(matches!(error, ExecutionError::NoSuchKey(_)), "{error}");
        }

        #[test]
        fn our_own_metric_matches_on_its_name() {
            assert!(matches(MATCHERS, &ours()).expect("it evaluates"));
        }

        #[test]
        fn a_library_metric_is_left_alone() {
            let metric: SampleMetric = parse(include_str!(
                "../fixtures/cel/metric-common/metric-http-client-request-duration.json"
            ));
            assert!(!matches(MATCHERS, &metric).expect("it evaluates"));
        }

        #[test]
        fn attributes_are_the_data_points_together() {
            let when =
                r#""myapp.checkout.stage" in attributes && "myapp.tenant.code" in attributes"#;
            assert!(evaluate(when, &ours()).expect("it evaluates"));
        }

        #[test]
        fn the_unit_and_instrument_are_bound() {
            assert!(
                evaluate(r#"unit == "s" && instrument == "histogram""#, &ours())
                    .expect("it evaluates")
            );
        }
    }

    /// A resource has only attributes.
    mod resource {
        use super::*;

        const MATCHERS: &str = include_str!("../fixtures/cel/resource/matchers.toml");

        fn resource(json: &str) -> SampleResource {
            parse(json)
        }

        #[test]
        fn our_own_service_matches() {
            let sample = resource(include_str!(
                "../fixtures/cel/resource/resource-myapp-checkout.json"
            ));
            assert!(matches(MATCHERS, &sample).expect("it evaluates"));
        }

        #[test]
        fn another_service_does_not() {
            let sample = resource(include_str!(
                "../fixtures/cel/resource/resource-other-service.json"
            ));
            assert!(!matches(MATCHERS, &sample).expect("it evaluates"));
        }

        /// The `in` guard prevents an error on the absent attribute.
        #[test]
        fn a_resource_without_a_service_name_does_not_error() {
            let sample = resource(include_str!(
                "../fixtures/cel/resource/resource-no-service-name.json"
            ));
            assert!(!matches(MATCHERS, &sample).expect("it evaluates"));
        }
    }

    /// A matcher keyed on the scope name.
    mod instrumentation_scope {
        use super::*;

        const MATCHERS: &str = include_str!("../fixtures/cel/instrumentation-scope/matchers.toml");

        #[test]
        fn our_own_instrumentation_matches_on_its_name() {
            let scope: SampleInstrumentationScope = parse(include_str!(
                "../fixtures/cel/instrumentation-scope/scope-myapp-checkout.json"
            ));
            assert!(matches(MATCHERS, &scope).expect("it evaluates"));
        }

        #[test]
        fn a_library_scope_does_not() {
            let scope: SampleInstrumentationScope = parse(include_str!(
                "../fixtures/cel/instrumentation-scope/scope-jdbc.json"
            ));
            assert!(!matches(MATCHERS, &scope).expect("it evaluates"));
        }
    }

    /// The ingester attaches the resource and scope, so the tests do too.
    mod signal_context {
        use super::*;

        fn span_from_scope(scope: Option<Rc<SampleInstrumentationScope>>) -> SampleSpan {
            let mut span: SampleSpan = parse(include_str!(
                "../fixtures/cel/span-checkout/span-checkout-payment.json"
            ));
            span.instrumentation_scope = scope;
            span.resource = Some(Rc::new(parse(include_str!(
                "../fixtures/cel/resource/resource-myapp-checkout.json"
            ))));
            span
        }

        fn scope(fixture: &str) -> Rc<SampleInstrumentationScope> {
            Rc::new(parse(fixture))
        }

        #[test]
        fn the_scope_of_a_signal_sample_is_bound() {
            let when = r#"instrumentation_scope.name.startsWith("myapp.")"#;
            let ours = span_from_scope(Some(scope(include_str!(
                "../fixtures/cel/instrumentation-scope/scope-myapp-checkout.json"
            ))));
            let theirs = span_from_scope(Some(scope(include_str!(
                "../fixtures/cel/instrumentation-scope/scope-jdbc.json"
            ))));
            assert!(evaluate(when, &ours).expect("it evaluates"));
            assert!(!evaluate(when, &theirs).expect("it evaluates"));
        }

        #[test]
        fn the_resource_of_a_signal_sample_is_bound() {
            let when = r#""service.name" in resource.attributes
                          && resource.attributes["service.name"] == "myapp.checkout""#;
            let span = span_from_scope(None);
            assert!(evaluate(when, &span).expect("it evaluates"));
        }

        #[test]
        fn an_unguarded_read_of_an_absent_scope_errors() {
            let span = span_from_scope(None);
            let _ = evaluate(r#"instrumentation_scope.name == "x""#, &span).expect_err("it errors");
        }

        #[test]
        fn an_absent_scope_is_null_and_can_be_guarded() {
            let span = span_from_scope(None);
            let when = r#"instrumentation_scope != null && instrumentation_scope.name == "x""#;
            assert!(!evaluate(when, &span).expect("it evaluates"));
        }

        #[test]
        fn an_absent_resource_is_null_and_can_be_guarded() {
            let mut span = span_from_scope(None);
            span.resource = None;
            let when = r#"resource != null && "service.name" in resource.attributes"#;
            assert!(!evaluate(when, &span).expect("it evaluates"));
        }
    }
}
