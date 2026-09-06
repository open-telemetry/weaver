// SPDX-License-Identifier: Apache-2.0

//! Library to hide details of jaq from the rest of weaver.

use std::{
    borrow::Cow,
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::error::Error;
use jaq_core::{
    data,
    load::{self, parse::Def, Arena, File, Import, Loader},
    val::unwrap_valr,
    Ctx, Vars,
};
use jaq_json::Val;

type JqFileType = PathBuf;

fn semconv_prelude() -> impl Iterator<Item = Def<&'static str>> {
    load::parse(crate::SEMCONV_JQ, |p| p.defs())
        .expect("BAD WEAVER BUILD - default JQ library failed to compile")
        .into_iter()
}

fn serde_to_val(v: serde_json::Value) -> Result<Val, Error> {
    serde_json::from_value(v)
        .map_err(|e| Error::InternalError(format!("failed to convert JSON value to jaq Val: {e}")))
}

fn val_to_serde(v: Val) -> serde_json::Value {
    serde_json::from_str(&v.to_string()).unwrap_or(serde_json::Value::Null)
}

fn prepare_jq_context(
    params: &BTreeMap<String, serde_json::Value>,
) -> Result<(Vec<String>, Vec<Val>), Error> {
    params
        .iter()
        .map(|(k, v)| Ok((format!("${k}"), serde_to_val(v.clone())?)))
        .collect::<Result<Vec<_>, _>>()
        .map(|pairs: Vec<_>| pairs.into_iter().unzip())
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
    execute_jq_with_modules(input, filter_expr, params, Path::new("."), &[])
}

/// Run a JQ filter with user-provided modules available as additional preludes.
///
/// Module paths are resolved relative to `module_base`; includes inside a
/// module are resolved relative to that module. Modules are loaded in list
/// order, so a later module takes precedence when it defines the same filter
/// as an earlier module or Weaver's built-in prelude.
pub fn execute_jq_with_modules(
    input: &serde_json::Value,
    filter_expr: &str,
    params: &BTreeMap<String, serde_json::Value>,
    module_base: &Path,
    modules: &[PathBuf],
) -> Result<serde_json::Value, Error> {
    if log::log_enabled!(log::Level::Trace) {
        log::trace!("Executing JQ filter: {filter_expr} with params {params:#?}, input {input:#?}");
    } else if log::log_enabled!(log::Level::Debug) {
        log::debug!("Executing JQ filter: {filter_expr} with params {params:#?}");
    }

    let loader = Loader::new(
        jaq_core::defs()
            .chain(jaq_std::defs())
            .chain(jaq_json::defs())
            .chain(semconv_prelude()),
    )
    .with_read(read_module);
    let program_code = if modules.is_empty() {
        Cow::Borrowed(filter_expr)
    } else {
        Cow::Owned(module_includes(filter_expr, modules)?)
    };
    let main_path = module_base.join(".weaver-jq-entrypoint.jq");
    let prelude_lines = modules.len();
    let arena = Arena::default();
    let program: File<&str, JqFileType> = File {
        code: &program_code,
        path: main_path.clone(),
    };

    // parse the filter
    let modules = loader
        .load(&arena, program)
        .map_err(|errs| load_errors(errs, &main_path, prelude_lines))
        .map_err(|details| Error::FilterError {
            filter: filter_expr.to_owned(),
            details,
        })?;

    let (names, values) = prepare_jq_context(params)?;
    let funs = jaq_core::funs()
        .chain(jaq_std::funs())
        .chain(jaq_json::funs());
    #[allow(clippy::map_identity)]
    let filter = jaq_core::Compiler::default()
        .with_global_vars(names.iter().map(|s| s.as_str()))
        // Re-borrow &'static str with shorter lifetime so 'global_vars lifetime is unified.
        // This is NOT a simple identity function — it's a lifetime inference workaround.
        .with_funs(funs.map(|x| x))
        .compile(modules)
        .map_err(|errs| compile_errors(errs, &main_path, prelude_lines))
        .map_err(|details| Error::FilterError {
            filter: filter_expr.to_owned(),
            details,
        })?;

    let ctx = Ctx::<data::JustLut<Val>>::new(&filter.lut, Vars::new(values));

    // Bundle Results
    let mut errs = Vec::new();
    let mut values = Vec::new();
    for r in filter
        .id
        .run((ctx, serde_to_val(input.clone())?))
        .map(unwrap_valr)
    {
        match r {
            Ok(v) => values.push(val_to_serde(v)),
            Err(e) => errs.push(e),
        }
    }

    if !errs.is_empty() {
        return Err(Error::FilterError {
            filter: filter_expr.to_owned(),
            details: errs
                .into_iter()
                .map(|e| FilterErrorDetail {
                    error: format!("{e}"),
                    file: None,
                    source: None,
                })
                .collect(),
        });
    }

    if log::log_enabled!(log::Level::Trace) {
        log::trace!(
            "JQ filter produced {} result(s): {}",
            values.len(),
            serde_json::Value::from(values.clone())
        );
    } else {
        log::debug!("JQ filter produced {} result(s)", values.len());
    }

    if values.len() == 1 {
        return Ok(values.pop().expect("values.len() == 1, should not happen"));
    }

    Ok(serde_json::Value::Array(values))
}

fn module_includes(filter_expr: &str, modules: &[PathBuf]) -> Result<String, Error> {
    modules
        .iter()
        .map(|module| -> Result<_, Error> {
            let module = serde_json::to_string(&module.to_string_lossy()).map_err(|e| {
                Error::InternalError(format!("failed to serialize JQ module path: {e}"))
            })?;
            Ok(format!("include {module};\n"))
        })
        .chain(std::iter::once(Ok(filter_expr.to_owned())))
        .collect()
}

fn read_module(import: Import<'_, &str, PathBuf>) -> Result<File<String, PathBuf>, String> {
    let requested_path = Path::new(import.path);
    let mut path = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        import
            .parent
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(requested_path)
    };
    _ = path.set_extension("jq");
    let path = path
        .canonicalize()
        .map_err(|_| "file not found".to_owned())?;
    let code = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(File { code, path })
}

// JAQ errors must be parsed and synthesized.  All of this code is adapted from `jaq/src/main.rs`.
use crate::error::{FilterErrorDetail, Location, Source};

fn get_source(whole: &str, part: &str) -> Option<Source> {
    let whole_start = whole.as_ptr() as usize;
    let whole_end = whole_start + whole.len();
    let part_start = part.as_ptr() as usize;
    let part_end = part_start + part.len();

    if part_start >= whole_start && part_end <= whole_end {
        let offset_start = part_start - whole_start;
        let offset_end = part_end - whole_start;

        let mut current_line = 1;
        let mut current_col = 1;

        for c in whole[..offset_start].chars() {
            if c == '\n' {
                current_line += 1;
                current_col = 1;
            } else {
                current_col += 1;
            }
        }
        let start = Location {
            line: current_line,
            col: current_col,
        };

        for c in whole[offset_start..offset_end].chars() {
            if c == '\n' {
                current_line += 1;
                current_col = 1;
            } else {
                current_col += 1;
            }
        }
        let end = Location {
            line: current_line,
            col: current_col,
        };

        Some(Source {
            start,
            end: Some(end),
        })
    } else {
        None
    }
}

/// Turns loading errors from jaq into structured details.
fn load_errors(
    errs: load::Errors<&str, JqFileType>,
    main_path: &Path,
    prelude_lines: usize,
) -> Vec<FilterErrorDetail> {
    use load::Error;
    errs.into_iter()
        .flat_map(|(file, err)| {
            let code = file.code;
            let path = file.path;
            let result: Vec<FilterErrorDetail> = match err {
                Error::Io(errs) => errs.into_iter().map(report_io).collect(),
                Error::Lex(errs) => errs
                    .into_iter()
                    .map(|e| report_lex(&path, main_path, prelude_lines, code, e))
                    .collect(),
                Error::Parse(errs) => errs
                    .into_iter()
                    .map(|e| report_parse(&path, main_path, prelude_lines, code, e))
                    .collect(),
            };
            result
        })
        .collect()
}

/// Turns compile errors from jaq into structured details.
fn compile_errors(
    errs: jaq_core::compile::Errors<&str, JqFileType>,
    main_path: &Path,
    prelude_lines: usize,
) -> Vec<FilterErrorDetail> {
    errs.into_iter()
        .flat_map(|(file, errs)| {
            let code = file.code;
            let path = file.path;
            errs.into_iter()
                .map(move |e| report_compile(&path, main_path, prelude_lines, code, e))
        })
        .collect()
}

/// Turns IO errors from JQ into structured details.
fn report_io((path, error): (&str, String)) -> FilterErrorDetail {
    FilterErrorDetail {
        error: format!("could not load file {path}: {error}"),
        file: None,
        source: None,
    }
}

/// Turns lexing errors from JQ into structured details.
fn report_lex(
    path: &Path,
    main_path: &Path,
    prelude_lines: usize,
    code: &str,
    (expected, span): load::lex::Error<&str>,
) -> FilterErrorDetail {
    FilterErrorDetail {
        error: format!("expected {}", expected.as_str()),
        file: (path != main_path).then(|| path.to_path_buf()),
        source: source_with_original_filter_lines(code, span, path == main_path, prelude_lines),
    }
}

/// Turns parsing errors from JQ into structured details.
fn report_parse(
    path: &Path,
    main_path: &Path,
    prelude_lines: usize,
    code: &str,
    (expected, span): load::parse::Error<&str>,
) -> FilterErrorDetail {
    FilterErrorDetail {
        error: format!("expected {}", expected.as_str()),
        file: (path != main_path).then(|| path.to_path_buf()),
        source: source_with_original_filter_lines(code, span, path == main_path, prelude_lines),
    }
}

/// Turns errors coming from JAQ compile phase into structured details.
fn report_compile(
    path: &Path,
    main_path: &Path,
    prelude_lines: usize,
    code: &str,
    (found, undefined): jaq_core::compile::Error<&str>,
) -> FilterErrorDetail {
    use jaq_core::compile::Undefined::Filter;
    let wnoa = |exp, got| format!("wrong number of arguments (expected {exp}, found {got})");
    let msg = match (found, undefined) {
        ("reduce", Filter(arity)) => wnoa("2", arity),
        ("foreach", Filter(arity)) => wnoa("2 or 3", arity),
        (_, undefined) => format!("undefined {}", undefined.as_str()),
    };
    FilterErrorDetail {
        error: msg,
        file: (path != main_path).then(|| path.to_path_buf()),
        source: source_with_original_filter_lines(code, found, path == main_path, prelude_lines),
    }
}

fn source_with_original_filter_lines(
    code: &str,
    part: &str,
    is_main_filter: bool,
    prelude_lines: usize,
) -> Option<Source> {
    let mut source = get_source(code, part)?;
    if is_main_filter {
        source.start.line = source.start.line.saturating_sub(prelude_lines);
        if let Some(end) = source.end.as_mut() {
            end.line = end.line.saturating_sub(prelude_lines);
        }
    }
    Some(source)
}

#[cfg(test)]
mod tests {
    use crate::error::Error;
    use serde_json::json;
    use std::{collections::BTreeMap, fs};

    use super::{execute_jq, execute_jq_with_modules};

    #[test]
    fn default_jq_execution_is_unchanged() {
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
    fn run_jq_with_user_modules() {
        let modules_dir = tempfile::tempdir().expect("Failed to create module directory");
        fs::write(
            modules_dir.path().join("custom.jq"),
            "def custom_value: .value + 1;",
        )
        .expect("Failed to write module");

        let result = execute_jq_with_modules(
            &json!({ "value": 41 }),
            "custom_value",
            &BTreeMap::new(),
            modules_dir.path(),
            &["custom.jq".into()],
        )
        .expect("Failed to execute JQ with user module");

        assert_eq!(result, json!(42));
    }

    #[test]
    fn user_modules_resolve_nested_relative_includes() {
        let modules_dir = tempfile::tempdir().expect("Failed to create module directory");
        let nested_dir = modules_dir.path().join("nested");
        fs::create_dir_all(&nested_dir).expect("Failed to create nested module directory");
        fs::write(
            modules_dir.path().join("custom.jq"),
            "include \"nested/helper\"; def custom_value: helper;",
        )
        .expect("Failed to write module");
        fs::write(nested_dir.join("helper.jq"), "def helper: 42;")
            .expect("Failed to write nested module");

        let result = execute_jq_with_modules(
            &json!({}),
            "custom_value",
            &BTreeMap::new(),
            modules_dir.path(),
            &["custom.jq".into()],
        )
        .expect("Failed to execute JQ with nested module");

        assert_eq!(result, json!(42));
    }

    #[test]
    fn user_modules_do_not_replace_the_semconv_prelude() {
        let modules_dir = tempfile::tempdir().expect("Failed to create module directory");
        fs::write(modules_dir.path().join("custom.jq"), "def custom_value: .;")
            .expect("Failed to write module");

        let result = execute_jq_with_modules(
            &json!({ "groups": [] }),
            "custom_value | semconv_attributes",
            &BTreeMap::new(),
            modules_dir.path(),
            &["custom.jq".into()],
        )
        .expect("Failed to execute JQ with built-in and user modules");

        assert_eq!(result, json!([]));
    }

    #[test]
    fn user_modules_can_intentionally_override_the_semconv_prelude() {
        let modules_dir = tempfile::tempdir().expect("Failed to create module directory");
        fs::write(
            modules_dir.path().join("custom.jq"),
            "def semconv_attributes: 42;",
        )
        .expect("Failed to write module");

        let result = execute_jq_with_modules(
            &json!({ "groups": [] }),
            "semconv_attributes",
            &BTreeMap::new(),
            modules_dir.path(),
            &["custom.jq".into()],
        )
        .expect("Failed to execute JQ with an overridden built-in");

        assert_eq!(result, json!(42));
    }

    #[test]
    fn later_user_modules_take_precedence_on_filter_collisions() {
        let modules_dir = tempfile::tempdir().expect("Failed to create module directory");
        fs::write(modules_dir.path().join("first.jq"), "def custom_value: 1;")
            .expect("Failed to write first module");
        fs::write(modules_dir.path().join("second.jq"), "def custom_value: 2;")
            .expect("Failed to write second module");

        let result = execute_jq_with_modules(
            &json!({}),
            "custom_value",
            &BTreeMap::new(),
            modules_dir.path(),
            &["first.jq".into(), "second.jq".into()],
        )
        .expect("Failed to execute JQ with colliding modules");

        assert_eq!(result, json!(2));
    }

    #[test]
    fn reports_missing_user_module() {
        let modules_dir = tempfile::tempdir().expect("Failed to create module directory");
        let error = execute_jq_with_modules(
            &json!({}),
            ".",
            &BTreeMap::new(),
            modules_dir.path(),
            &["missing.jq".into()],
        )
        .expect_err("Missing module should fail");

        assert!(format!("{error}").contains("could not load file missing.jq: file not found"));
    }

    #[test]
    fn reports_invalid_user_module() {
        let modules_dir = tempfile::tempdir().expect("Failed to create module directory");
        let module_path = modules_dir.path().join("broken.jq");
        fs::write(&module_path, "def broken: (").expect("Failed to write module");

        let error = execute_jq_with_modules(
            &json!({}),
            ".",
            &BTreeMap::new(),
            modules_dir.path(),
            &["broken.jq".into()],
        )
        .expect_err("Invalid module should fail");

        let Error::FilterError { details, .. } = error else {
            panic!("Expected a filter error");
        };
        assert!(details[0].error.contains("expected closing parenthesis"));
        assert_eq!(
            details[0].file,
            Some(
                module_path
                    .canonicalize()
                    .expect("Failed to canonicalize module path")
            )
        );
    }

    #[test]
    fn user_modules_do_not_shift_filter_error_locations() {
        let modules_dir = tempfile::tempdir().expect("Failed to create module directory");
        fs::write(modules_dir.path().join("custom.jq"), "def custom_value: .;")
            .expect("Failed to write module");

        let error = execute_jq_with_modules(
            &json!({}),
            "(",
            &BTreeMap::new(),
            modules_dir.path(),
            &["custom.jq".into()],
        )
        .expect_err("Invalid filter should fail");

        let Error::FilterError { details, .. } = error else {
            panic!("Expected a filter error");
        };
        assert!(details[0].file.is_none());
        assert_eq!(
            details[0].source.as_ref().map(|source| source.start.line),
            Some(1)
        );
    }

    #[test]
    fn test_lex_error() {
        let input = json!({});
        let values = BTreeMap::new();
        let error = execute_jq(&input, "(", &values).expect_err("Should have failed to lex");
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
            .expect_err("Should have failed to parse");
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
        let error =
            execute_jq(&input, ".x | de", &values).expect_err("Should have failed to parse");
        let msg = format!("{error}");
        assert!(
            msg.contains("undefined filter"),
            "Expected compile error {msg}"
        );
    }

    #[test]
    fn test_cannot_iterate_error() {
        let input = json!(["a", "b"]);
        let values = BTreeMap::new();
        let error =
            execute_jq(&input, ".[] | unique", &values).expect_err("Should have failed to execute");
        let msg = format!("{error}");

        assert!(
            msg.contains("cannot use \"a\" as array"),
            "Expected execute error, but got {msg}"
        );
        assert!(
            msg.contains("cannot use \"b\" as array"),
            "Expected execute error, but got '{msg}'"
        );
    }

    #[test]
    fn test_report_io_error() {
        let detail = super::report_io(("test_file.jq", "permission denied".to_owned()));
        assert_eq!(
            detail.error,
            "could not load file test_file.jq: permission denied"
        );
        assert!(detail.source.is_none());
    }

    #[test]
    fn test_non_json_number_does_not_panic() {
        // "00.0" triggers Val::Num with a leading-zero string that serde_json rejects.
        // Previously this caused a panic in jaq_json 1.x via an unwrap().
        let input = json!({});
        let values = BTreeMap::new();
        let _ = execute_jq(&input, "00.0", &values);
    }
}
