// SPDX-License-Identifier: Apache-2.0

//! Runs CEL expressions against live-check samples.
//!
//! Each sample type implements `Matchable` in its own module. It adds its
//! variables to a `Context` with the serde support of the `cel` crate. So a
//! field keeps its serde form: an enum is its variant name, and an `Option`
//! is the value or `null`.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use cel::{Context, Env, ExecutionError, Program, SerializationError, Value};
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::{
    sample_attribute::SampleAttribute, sample_instrumentation_scope::SampleInstrumentationScope,
    sample_resource::SampleResource, SampleType,
};

/// A sample the matchers can select.
pub trait Matchable {
    /// The kind of sample, which decides which matchers apply to it.
    fn sample_type(&self) -> SampleType;

    /// Adds the variables of the sample to a CEL context.
    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError>;
}

/// The CEL standard library, built once. `Context::default` builds it again
/// on every call.
static STDLIB: OnceLock<Arc<Env>> = OnceLock::new();

/// A context with the standard library and no variables.
pub(crate) fn stdlib_context() -> Context<'static> {
    Context::with_env(Arc::clone(STDLIB.get_or_init(|| Arc::new(Env::stdlib()))))
}

/// Runs a compiled `when` against a context.
///
/// # Errors
///
/// Returns the error from the interpreter, or `UnexpectedType` when the
/// result is not a bool.
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
pub(crate) type AttributeMap<'a> = HashMap<&'a str, &'a Option<JsonValue>>;

pub(crate) fn attribute_map<'a>(
    attributes: impl Iterator<Item = &'a SampleAttribute>,
) -> AttributeMap<'a> {
    attributes
        .map(|attribute| (attribute.name.as_str(), &attribute.value))
        .collect()
}

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

/// Binds `resource` and `instrumentation_scope`. If the sample has no
/// resource or no scope, that variable is `null`.
pub(crate) fn bind_signal_context(
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

/// Compiles `when` and runs it against one sample.
#[cfg(test)]
pub(crate) fn evaluate(when: &str, sample: &dyn Matchable) -> Result<bool, ExecutionError> {
    let program = Program::compile(when).expect("the expression compiles");
    let mut context = stdlib_context();
    sample.bind(&mut context).expect("the sample binds");
    execute(&program, &context)
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::sample_span::SampleSpan;

    fn parse<T: serde::de::DeserializeOwned>(json: &str) -> T {
        serde_json::from_str(json).expect("the fixture parses")
    }

    fn span() -> SampleSpan {
        parse(include_str!(
            "../fixtures/cel/span-checkout/span-checkout-payment.json"
        ))
    }

    #[test]
    fn an_expression_that_is_not_a_bool_is_rejected() {
        let error =
            evaluate(r#"attributes["myapp.checkout.stage"]"#, &span()).expect_err("not a bool");
        assert!(
            matches!(error, ExecutionError::UnexpectedType { .. }),
            "{error}"
        );
    }

    /// There is no startup check for this. It is an error on every sample.
    #[test]
    fn a_variable_the_sample_type_does_not_have_errors() {
        let error = evaluate(r#"unit == "s""#, &span()).expect_err("it errors");
        assert!(
            matches!(error, ExecutionError::UndeclaredReference(_)),
            "{error}"
        );
    }

    /// The docs promise this. An unguarded read of an absent key errors, and
    /// the `in` guard works on either side of the `&&`.
    #[test]
    fn a_guard_works_on_either_side_of_an_and() {
        let span = span();
        let _ = evaluate(r#"attributes["absent"] == "x""#, &span).expect_err("it errors");
        for when in [
            r#""absent" in attributes && attributes["absent"] == "x""#,
            r#"attributes["absent"] == "x" && "absent" in attributes"#,
        ] {
            assert!(!evaluate(when, &span).expect("it evaluates"), "{when}");
        }
    }

    /// A positive JSON integer becomes a CEL `uint`. The comparison with an
    /// `int` literal relies on the crate comparing across numeric types.
    #[test]
    fn attribute_values_keep_their_json_types() {
        let span: SampleSpan = parse(
            r#"{
              "name": "typed", "kind": "internal", "status": null,
              "attributes": [
                { "name": "s", "value": "x" },
                { "name": "i", "value": 3 },
                { "name": "f", "value": 0.5 },
                { "name": "b", "value": true },
                { "name": "l", "value": ["a", "b"] },
                { "name": "n", "value": null }
              ],
              "live_check_result": null
            }"#,
        );
        let when = r#"attributes["s"] == "x"
            && attributes["i"] == 3 && attributes["i"] > 2
            && attributes["f"] < 1.0
            && attributes["b"]
            && "a" in attributes["l"]
            && attributes["n"] == null"#;
        assert!(evaluate(when, &span).expect("it evaluates"));
    }

    #[test]
    fn a_signal_sample_binds_its_resource_and_scope() {
        let mut span = span();
        span.resource = Some(Rc::new(parse(include_str!(
            "../fixtures/cel/resource/resource-myapp-checkout.json"
        ))));
        span.instrumentation_scope = Some(Rc::new(parse(include_str!(
            "../fixtures/cel/instrumentation-scope/scope-myapp-checkout.json"
        ))));
        let when = r#"resource.attributes["service.name"] == "myapp.checkout"
            && instrumentation_scope.name == "myapp.checkout.instrumentation"
            && instrumentation_scope.version == "0.3.1"
            && instrumentation_scope.schema_url == "https://example.com/myschema/1.0.0"
            && instrumentation_scope.attributes["myapp.instrumentation.mode"] == "auto""#;
        assert!(evaluate(when, &span).expect("it evaluates"));
    }

    /// The docs promise this too: a `!= null` guard prevents the error.
    #[test]
    fn an_absent_resource_or_scope_is_null() {
        let span = span();
        assert!(
            evaluate("resource == null && instrumentation_scope == null", &span)
                .expect("it evaluates")
        );
        let _ = evaluate(r#"instrumentation_scope.name == "x""#, &span).expect_err("it errors");
        let when = r#"(resource != null && "service.name" in resource.attributes)
            || (instrumentation_scope != null && instrumentation_scope.name == "x")"#;
        assert!(!evaluate(when, &span).expect("it evaluates"));
    }
}
