// SPDX-License-Identifier: Apache-2.0

//! Binds live-check samples to CEL variables.
//!
//! Each sample type offers its own set of variables, and only the ones an
//! expression reads are bound.

use std::collections::HashMap;

use serde_json::Value as JsonValue;
use weaver_cel::{Bindings, Context, Referenced, Value};

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

/// The `resource` variable, offered by every signal sample.
const RESOURCE: &str = "resource";
/// The `instrumentation_scope` variable, offered by every signal sample.
const INSTRUMENTATION_SCOPE: &str = "instrumentation_scope";

/// The variables an expression can read for a sample type.
///
/// Empty means expressions never run against that sample type.
#[must_use]
pub fn variables(sample_type: SampleType) -> &'static [&'static str] {
    match sample_type {
        SampleType::Span => &[
            "name",
            "kind",
            "status",
            "attributes",
            RESOURCE,
            INSTRUMENTATION_SCOPE,
        ],
        SampleType::SpanEvent => &["name", "attributes", RESOURCE, INSTRUMENTATION_SCOPE],
        SampleType::SpanLink => &["attributes", RESOURCE, INSTRUMENTATION_SCOPE],
        SampleType::Log => &[
            "event_name",
            "severity_text",
            "severity_number",
            "body",
            "attributes",
            RESOURCE,
            INSTRUMENTATION_SCOPE,
        ],
        SampleType::Metric => &[
            "name",
            "unit",
            "instrument",
            "attributes",
            RESOURCE,
            INSTRUMENTATION_SCOPE,
        ],
        SampleType::Profile => &["attributes", RESOURCE, INSTRUMENTATION_SCOPE],
        SampleType::Resource => &["attributes"],
        SampleType::InstrumentationScope => &["name", "version", "schema_url", "attributes"],
        SampleType::Attribute
        | SampleType::NumberDataPoint
        | SampleType::HistogramDataPoint
        | SampleType::ExponentialHistogramDataPoint
        | SampleType::Exemplar => &[],
    }
}

impl Bindings for SampleSpan {
    fn bind(&self, referenced: &Referenced, context: &mut Context<'_>) {
        if referenced.wants("name") {
            context.add_variable_from_value("name", self.name.as_str());
        }
        if referenced.wants("kind") {
            context.add_variable_from_value("kind", enum_name(&self.kind));
        }
        if referenced.wants("status") {
            // OTLP treats a missing status as unset.
            let (code, message) = match &self.status {
                Some(status) => (enum_name(&status.code), status.message.as_str()),
                None => ("unset".to_owned(), ""),
            };
            context.add_variable_from_value(
                "status",
                HashMap::from([
                    ("code".to_owned(), Value::from(code)),
                    ("message".to_owned(), Value::from(message)),
                ]),
            );
        }
        bind_attributes(&self.attributes, referenced, context);
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            referenced,
            context,
        );
    }
}

impl Bindings for SampleSpanEvent {
    fn bind(&self, referenced: &Referenced, context: &mut Context<'_>) {
        if referenced.wants("name") {
            context.add_variable_from_value("name", self.name.as_str());
        }
        bind_attributes(&self.attributes, referenced, context);
    }
}

impl Bindings for SampleSpanLink {
    fn bind(&self, referenced: &Referenced, context: &mut Context<'_>) {
        bind_attributes(&self.attributes, referenced, context);
    }
}

impl Bindings for SampleLog {
    fn bind(&self, referenced: &Referenced, context: &mut Context<'_>) {
        if referenced.wants("event_name") {
            context.add_variable_from_value("event_name", self.event_name.as_str());
        }
        if referenced.wants("severity_text") {
            context.add_variable_from_value(
                "severity_text",
                self.severity_text.as_deref().unwrap_or_default(),
            );
        }
        if referenced.wants("severity_number") {
            context.add_variable_from_value(
                "severity_number",
                i64::from(self.severity_number.unwrap_or_default()),
            );
        }
        if referenced.wants("body") {
            context.add_variable_from_value("body", self.body.as_deref().unwrap_or_default());
        }
        bind_attributes(&self.attributes, referenced, context);
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            referenced,
            context,
        );
    }
}

impl Bindings for SampleMetric {
    fn bind(&self, referenced: &Referenced, context: &mut Context<'_>) {
        if referenced.wants("name") {
            context.add_variable_from_value("name", self.name.as_str());
        }
        if referenced.wants("unit") {
            context.add_variable_from_value("unit", self.unit.as_str());
        }
        if referenced.wants("instrument") {
            context.add_variable_from_value("instrument", enum_name(&self.instrument));
        }
        if referenced.wants("attributes") {
            // Metric attributes are the union over the data points.
            context
                .add_variable_from_value("attributes", attribute_map(data_point_attributes(self)));
        }
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            referenced,
            context,
        );
    }
}

impl Bindings for SampleResource {
    fn bind(&self, referenced: &Referenced, context: &mut Context<'_>) {
        bind_attributes(&self.attributes, referenced, context);
    }
}

impl Bindings for SampleInstrumentationScope {
    fn bind(&self, referenced: &Referenced, context: &mut Context<'_>) {
        if referenced.wants("name") {
            context.add_variable_from_value("name", self.name.as_str());
        }
        if referenced.wants("version") {
            context.add_variable_from_value("version", self.version.as_str());
        }
        if referenced.wants("schema_url") {
            context.add_variable_from_value("schema_url", self.schema_url.as_str());
        }
        bind_attributes(&self.attributes, referenced, context);
    }
}

impl Bindings for SampleProfile {
    fn bind(&self, referenced: &Referenced, context: &mut Context<'_>) {
        bind_attributes(&self.attributes, referenced, context);
        bind_signal_context(
            self.resource.as_deref(),
            self.instrumentation_scope.as_deref(),
            referenced,
            context,
        );
    }
}

/// All attributes from all data points of a metric.
fn data_point_attributes(metric: &SampleMetric) -> Box<dyn Iterator<Item = &SampleAttribute> + '_> {
    match &metric.data_points {
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
    }
}

fn bind_attributes(
    attributes: &[SampleAttribute],
    referenced: &Referenced,
    context: &mut Context<'_>,
) {
    if referenced.wants("attributes") {
        context.add_variable_from_value("attributes", attribute_map(attributes.iter()));
    }
}

/// Binds `resource` and `instrumentation_scope`.
///
/// An absent one is left unbound, so reading it errors.
fn bind_signal_context(
    resource: Option<&SampleResource>,
    scope: Option<&SampleInstrumentationScope>,
    referenced: &Referenced,
    context: &mut Context<'_>,
) {
    if referenced.wants(RESOURCE) {
        if let Some(resource) = resource {
            context.add_variable_from_value(
                RESOURCE,
                HashMap::from([(
                    "attributes".to_owned(),
                    Value::from(attribute_map(resource.attributes.iter())),
                )]),
            );
        }
    }
    if referenced.wants(INSTRUMENTATION_SCOPE) {
        if let Some(scope) = scope {
            context.add_variable_from_value(
                INSTRUMENTATION_SCOPE,
                HashMap::from([
                    ("name".to_owned(), Value::from(scope.name.as_str())),
                    ("version".to_owned(), Value::from(scope.version.as_str())),
                    (
                        "schema_url".to_owned(),
                        Value::from(scope.schema_url.as_str()),
                    ),
                    (
                        "attributes".to_owned(),
                        Value::from(attribute_map(scope.attributes.iter())),
                    ),
                ]),
            );
        }
    }
}

/// Builds the map for `attributes["key"]`.
fn attribute_map<'a>(
    attributes: impl Iterator<Item = &'a SampleAttribute>,
) -> HashMap<String, Value> {
    attributes
        .map(|attribute| {
            let value = attribute.value.as_ref().map_or(Value::Null, json_to_cel);
            (attribute.name.clone(), value)
        })
        .collect()
}

fn json_to_cel(value: &JsonValue) -> Value {
    match value {
        JsonValue::Null => Value::Null,
        JsonValue::Bool(v) => Value::Bool(*v),
        JsonValue::Number(v) => v
            .as_i64()
            .map_or_else(|| Value::Float(v.as_f64().unwrap_or_default()), Value::Int),
        JsonValue::String(v) => Value::String(v.clone().into()),
        JsonValue::Array(v) => Value::List(v.iter().map(json_to_cel).collect::<Vec<_>>().into()),
        JsonValue::Object(v) => Value::from(
            v.iter()
                .map(|(key, value)| (key.clone(), json_to_cel(value)))
                .collect::<HashMap<_, _>>(),
        ),
    }
}

/// Serializes an enum value to its serde name.
fn enum_name<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(JsonValue::String(name)) => name,
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use serde::Deserialize;
    use weaver_cel::{Error, Expression};

    use super::*;

    /// A `[[live-check.matchers]]` table. Test-only until it has a home in
    /// the config.
    #[derive(Debug, Deserialize)]
    struct MatcherConfig {
        id: String,
        sample_type: String,
        when: Option<String>,
        signal: Option<String>,
        #[serde(default)]
        attribute_groups: Vec<String>,
    }

    /// The one matcher in a fixture config, with its expression compiled.
    fn matcher(fixture: &str) -> (MatcherConfig, Option<Expression>) {
        #[derive(Debug, Deserialize)]
        struct Fixture {
            #[serde(rename = "live-check")]
            live_check: LiveCheck,
        }
        #[derive(Debug, Deserialize)]
        struct LiveCheck {
            matchers: Vec<MatcherConfig>,
        }

        let fixture: Fixture = toml::from_str(fixture).expect("the fixture config parses");
        let mut matchers = fixture.live_check.matchers;
        assert_eq!(matchers.len(), 1, "the fixture declares one matcher");
        let config = matchers.pop().expect("there is one matcher");
        let expression = config
            .when
            .as_deref()
            .map(|when| Expression::compile(when).expect("the fixture expressions compile"));
        (config, expression)
    }

    /// Evaluates the `when` of a fixture against a sample.
    fn matches(fixture: &str, sample: &dyn Bindings) -> Result<bool, Error> {
        let (_, expression) = matcher(fixture);
        // A fixture with no `when` matches every sample of the type.
        expression.map_or(Ok(true), |expression| expression.evaluate(sample))
    }

    fn parse<T: serde::de::DeserializeOwned>(json: &str) -> T {
        serde_json::from_str(json).expect("the fixture sample parses")
    }

    fn evaluate(when: &str, sample: &dyn Bindings) -> Result<bool, Error> {
        Expression::compile(when)
            .expect("it compiles")
            .evaluate(sample)
    }

    /// A span matched on a signature of attributes.
    mod span_checkout {
        use super::*;

        const MATCHERS: &str = include_str!("../fixtures/cel/span-checkout/matchers.toml");

        #[test]
        fn config_is_read_from_toml() {
            let (config, expression) = matcher(MATCHERS);
            assert_eq!(config.id, "myapp.checkout");
            assert_eq!(config.sample_type, "span");
            assert_eq!(config.signal.as_deref(), Some("myapp.checkout"));
            assert!(config.attribute_groups.is_empty());
            assert!(expression
                .expect("it has a when")
                .referenced()
                .wants("attributes"));
        }

        #[test]
        fn signature_present_and_stage_expected() {
            let span: SampleSpan = parse(include_str!(
                "../fixtures/cel/span-checkout/span-checkout-payment.json"
            ));
            assert!(matches(MATCHERS, &span).expect("it evaluates"));
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
            let span: SampleSpan = parse(include_str!(
                "../fixtures/cel/span-checkout/span-checkout-payment.json"
            ));
            assert!(evaluate(r#"kind == "internal""#, &span).expect("it evaluates"));
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
        }

        #[test]
        fn the_status_message_is_readable() {
            let sample = span(include_str!("../fixtures/cel/span-status/span-error.json"));
            assert!(
                evaluate(r#"status.message.contains("declined")"#, &sample).expect("it evaluates")
            );
        }

        /// A `when` of `"error.type" in attributes` would be circular, so the
        /// condition needs the status.
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

        #[test]
        fn config_adds_a_group_and_no_signal() {
            let (config, expression) = matcher(MATCHERS);
            assert_eq!(config.sample_type, "log");
            assert!(config.when.is_none());
            assert!(config.signal.is_none());
            assert_eq!(config.attribute_groups, ["myapp.common"]);
            assert!(expression.is_none());
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

        /// The `in` test keeps this from erroring.
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

        /// The variable is unbound, not empty.
        #[test]
        fn a_sample_with_no_scope_errors() {
            let span = span_from_scope(None);
            let error =
                evaluate(r#"instrumentation_scope.name == "x""#, &span).expect_err("it errors");
            assert!(matches!(error, Error::EvalFailed { .. }), "{error}");
        }
    }

    /// The check a startup lint would make.
    #[test]
    fn the_fixture_expressions_only_read_variables_that_exist() {
        let fixtures = [
            (
                SampleType::Span,
                include_str!("../fixtures/cel/span-checkout/matchers.toml"),
            ),
            (
                SampleType::Span,
                include_str!("../fixtures/cel/span-status/matchers.toml"),
            ),
            (
                SampleType::Log,
                include_str!("../fixtures/cel/log-common/matchers.toml"),
            ),
            (
                SampleType::Metric,
                include_str!("../fixtures/cel/metric-common/matchers.toml"),
            ),
            (
                SampleType::Resource,
                include_str!("../fixtures/cel/resource/matchers.toml"),
            ),
            (
                SampleType::InstrumentationScope,
                include_str!("../fixtures/cel/instrumentation-scope/matchers.toml"),
            ),
        ];
        for (sample_type, fixture) in fixtures {
            let (config, expression) = matcher(fixture);
            let Some(expression) = expression else {
                continue;
            };
            let allowed = variables(sample_type);
            for variable in expression.referenced().variables() {
                assert!(
                    allowed.contains(&variable),
                    "matcher `{}` reads `{variable}`, which {sample_type:?} does not offer",
                    config.id
                );
            }
        }
    }

    #[test]
    fn a_sample_type_a_matcher_cannot_target_offers_nothing() {
        assert!(variables(SampleType::NumberDataPoint).is_empty());
        assert!(variables(SampleType::Attribute).is_empty());
    }
}
