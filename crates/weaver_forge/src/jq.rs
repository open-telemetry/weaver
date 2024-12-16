// SPDX-License-Identifier: Apache-2.0

//! Library to hide details of jaq from the rest of weaver.

use std::collections::BTreeMap;

use crate::error::Error;
use jaq_core::{
    load::{parse::Def, Arena, File, Loader},
    Ctx, Native, RcIter,
};
use jaq_json::Val;

type JqFileType = ();

fn semconv_prelude() -> impl Iterator<Item = Def<&'static str>> {
    jaq_core::load::parse(crate::SEMCONV_JQ, |p| p.defs())
        .expect("BAD WEAVER BUILD - default JQ library failed to compile")
        .into_iter()
}

fn prepare_jq_context(params: &BTreeMap<String, serde_json::Value>) -> (Vec<String>, Vec<Val>) {
    let (jq_vars, jq_ctx): (Vec<String>, Vec<Val>) = params
        .iter()
        .map(|(k, v)| (format!("${k}"), Val::from(v.clone())))
        .unzip();
    (jq_vars, jq_ctx)
}

/// This is our single entry point for calling into the jaq library to run jq filters.
pub fn execute_jq(
    // The JSON input to JQ.
    input: &serde_json::Value,
    // The JQ filter to compile.
    filter_expr: &str,
    // Note: This will be exposed with `${key}` as the variable name.
    params: &BTreeMap<String, serde_json::Value>,
) -> Result<serde_json::Value, Error> {
    let loader = Loader::new(
        // ToDo: Allow custom preludes?
        jaq_std::defs()
            .chain(jaq_json::defs())
            .chain(semconv_prelude()), // [],
    );
    let arena = Arena::default();
    let program: File<&str, JqFileType> = File {
        code: filter_expr,
        path: (), // ToDo - give this the weaver-config location.
    };

    // parse the filter
    let modules = loader
        .load(&arena, program)
        .map_err(load_errors)
        .map_err(|e| Error::FilterError {
            filter: filter_expr.to_owned(),
            error: e,
        })?;

    let (names, values) = prepare_jq_context(params);
    let funs = jaq_std::funs().chain(jaq_json::funs());
    #[allow(clippy::map_identity)]
    let filter = jaq_core::Compiler::<_, Native<_>>::default()
        .with_global_vars(names.iter().map(|s| s.as_str()))
        // To trick compiler, we re-borrow `&'static str` with shorter lifetime.
        // This is *NOT* a simple identity function, but a lifetime inference workaround.
        .with_funs(funs.map(|x| x))
        .compile(modules)
        .map_err(compile_errors)
        .map_err(|e| Error::FilterError {
            filter: filter_expr.to_owned(),
            error: e,
        })?;
    let inputs = RcIter::new(core::iter::empty());
    let ctx = Ctx::new(values, &inputs);

    // Bundle Results
    let mut errs = Vec::new();
    let mut values = Vec::new();
    let filter_result = filter.run((ctx, Val::from(input.clone())));
    for r in filter_result {
        match r {
            Ok(v) => values.push(serde_json::Value::from(v)),
            Err(e) => errs.push(e),
        }
    }

    if values.len() == 1 {
        return Ok(values.pop().expect("values.len() == 1, should not happen"));
    }

    Ok(serde_json::Value::Array(values))
}

// JAQ errors must be parsed and synthesized.  All of this code is adapted from `jaq/src/main.rs`.

/// Converts all errors from jaq into a single string.
fn errors_to_string<Reports: Iterator<Item = String>>(reports: Reports) -> String {
    reports.into_iter().collect()
}

/// Turns loading errors from jaq into raw strings.
fn load_errors(errs: jaq_core::load::Errors<&str, JqFileType>) -> String {
    use jaq_core::load::Error;
    let errs = errs.into_iter().flat_map(|(_, err)| {
        let result: Vec<String> = match err {
            Error::Io(errs) => errs.into_iter().map(report_io).collect(),
            Error::Lex(errs) => errs.into_iter().map(report_lex).collect(),
            Error::Parse(errs) => errs.into_iter().map(report_parse).collect(),
        };
        result
    });
    errors_to_string(errs)
}

/// Turns compile errors from jaq into raw strings.
fn compile_errors(errs: jaq_core::compile::Errors<&str, JqFileType>) -> String {
    let errs = errs
        .into_iter()
        .flat_map(|(_, errs)| errs.into_iter().map(report_compile));
    errors_to_string(errs)
}

/// Turns IO errors from JQ into raw strings.
fn report_io((path, error): (&str, String)) -> String {
    format!("could not load file {}: {}", path, error)
}

/// Turns lexing errors from JQ into raw strings.
fn report_lex((expected, _): jaq_core::load::lex::Error<&str>) -> String {
    format!("expected {}", expected.as_str())
}

/// Turns parsing errors from JQ into raw strings.
fn report_parse((expected, _): jaq_core::load::parse::Error<&str>) -> String {
    format!("expected {}", expected.as_str())
}

/// Turns errors coming from JAQ compile phase into raw strings.
fn report_compile((found, undefined): jaq_core::compile::Error<&str>) -> String {
    use jaq_core::compile::Undefined::Filter;
    let wnoa = |exp, got| format!("wrong number of arguments (expected {exp}, found {got})");
    match (found, undefined) {
        ("reduce", Filter(arity)) => wnoa("2", arity),
        ("foreach", Filter(arity)) => wnoa("2 or 3", arity),
        (_, undefined) => format!("undefined {}", undefined.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use std::collections::BTreeMap;

    use super::execute_jq;

    #[test]
    fn run_jq() {
        let input = json!({
            "key1": 1,
            "key2": 2,
        });
        let values = BTreeMap::new();
        let result = execute_jq(&input, ".", &values).unwrap();
        assert_eq!(input, result);
    }

    #[test]
    fn run_jq_with_context() {
        let input = json!({
            "key1": 1,
            "key2": 2,
        });
        let values = BTreeMap::from([(
            "ctx1".to_owned(),
            json!({
                "key3": 3,
            }),
        )]);
        let result = execute_jq(&input, "$ctx1", &values).unwrap();
        assert_eq!(result, values["ctx1"]);
    }

    #[test]
    fn test_lex_error() {
        let input = json!({});
        let values = BTreeMap::new();
        let error = execute_jq(&input, "(", &values)
            .err()
            .expect("Should have failed to lex");
        let msg = format!("{error}");
        assert!(
            msg.contains("expected closing parenthesis"),
            "Expected lex error {msg}"
        );
    }

    #[test]
    fn test_parse_error() {
        let input = json!({});
        let values = BTreeMap::new();
        let error = execute_jq(&input, "if false then .", &values)
            .err()
            .expect("Should have failed to parse");
        let msg = format!("{error}");
        assert!(
            msg.contains("expected else or end"),
            "Expected parse error {msg}"
        );
    }

    #[test]
    fn test_compile_error() {
        let input = json!({});
        let values = BTreeMap::new();
        let error = execute_jq(&input, ".x | de", &values)
            .err()
            .expect("Should have failed to parse");
        let msg = format!("{error}");
        assert!(
            msg.contains("undefined filter"),
            "Expected compile error {msg}"
        );
    }
}
