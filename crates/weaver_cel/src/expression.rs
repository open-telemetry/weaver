// SPDX-License-Identifier: Apache-2.0

//! Compiled expressions.

use std::collections::BTreeSet;
use std::sync::Arc;

use cel::{Context, Env, Program, Value};

use crate::matches::literal_patterns;
use crate::{free_variables::free_variables, Bindings, Error};

thread_local! {
    /// The CEL standard environment, built once per thread because
    /// `Context::default` rebuilds the whole function table.
    static STDLIB: Arc<Env> = Arc::new(Env::stdlib());
}

/// Variables bound once, for evaluating several expressions against one sample.
pub struct Scope<'a> {
    context: Context<'a>,
}

impl Scope<'_> {
    /// Binds the variables in `referenced`.
    #[must_use]
    pub fn new(referenced: &Referenced, bindings: &dyn Bindings) -> Self {
        let mut context = Context::Root {
            env: STDLIB.with(Arc::clone),
            variables: Default::default(),
            functions: Default::default(),
            resolver: None,
        };
        bindings.bind(referenced, &mut context);
        Self { context }
    }
}

/// A compiled expression. Compiled once, evaluated per sample.
#[derive(Debug)]
pub struct Expression {
    source: String,
    program: Program,
    referenced: Referenced,
}

impl Expression {
    /// Compiles an expression.
    pub fn compile(source: &str) -> Result<Self, Error> {
        let program = Program::compile(source).map_err(|error| Error::CompileFailed {
            expression: source.to_owned(),
            error: error.to_string(),
        })?;
        for pattern in literal_patterns(program.expression()) {
            let _ = regex::Regex::new(pattern).map_err(|error| Error::BadPattern {
                expression: source.to_owned(),
                pattern: pattern.to_owned(),
                error: error.to_string(),
            })?;
        }
        let referenced = Referenced {
            variables: free_variables(program.expression()),
        };
        Ok(Self {
            source: source.to_owned(),
            program,
            referenced,
        })
    }

    /// The source text the expression was compiled from.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The variables the expression reads.
    #[must_use]
    pub fn referenced(&self) -> &Referenced {
        &self.referenced
    }

    /// Evaluates the expression against a set of bindings.
    ///
    /// Reading an absent map key or an unbound variable is an error, not
    /// false.
    pub fn evaluate(&self, bindings: &dyn Bindings) -> Result<bool, Error> {
        self.evaluate_in(&Scope::new(&self.referenced, bindings))
    }

    /// Evaluates the expression against variables a scope already bound.
    ///
    /// # Errors
    ///
    /// Returns an error when the expression fails to evaluate, or returns a
    /// value that is not a bool.
    pub fn evaluate_in(&self, scope: &Scope<'_>) -> Result<bool, Error> {
        let value = self
            .program
            .execute(&scope.context)
            .map_err(|error| Error::EvalFailed {
                expression: self.source.clone(),
                error: error.to_string(),
            })?;
        match value {
            Value::Bool(result) => Ok(result),
            other => Err(Error::NotBoolean {
                expression: self.source.clone(),
                value_type: other.type_of().to_string(),
            }),
        }
    }
}

/// The variables an expression reads, collected when it is compiled.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Referenced {
    variables: BTreeSet<String>,
}

impl Referenced {
    /// The variables read by any of these expressions.
    #[must_use]
    pub fn union<'a>(expressions: impl Iterator<Item = &'a Expression>) -> Self {
        Self {
            variables: expressions
                .flat_map(|expression| expression.referenced.variables.iter().cloned())
                .collect(),
        }
    }

    /// Whether the expression reads this variable.
    #[must_use]
    pub fn wants(&self, variable: &str) -> bool {
        self.variables.contains(variable)
    }

    /// The variables the expression reads, in name order.
    pub fn variables(&self) -> impl Iterator<Item = &str> {
        self.variables.iter().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::Scope;

    /// A stand-in sample: a name, some attributes, and a record of what was bound.
    struct TestBindings {
        name: &'static str,
        attributes: Vec<(&'static str, &'static str)>,
        bound: std::cell::RefCell<Vec<&'static str>>,
    }

    impl TestBindings {
        fn new(name: &'static str, attributes: Vec<(&'static str, &'static str)>) -> Self {
            Self {
                name,
                attributes,
                bound: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl Bindings for TestBindings {
        fn bind(&self, referenced: &Referenced, context: &mut Context<'_>) {
            if referenced.wants("name") {
                self.bound.borrow_mut().push("name");
                context.add_variable_from_value("name", self.name);
            }
            if referenced.wants("attributes") {
                self.bound.borrow_mut().push("attributes");
                let attributes: HashMap<String, Value> = self
                    .attributes
                    .iter()
                    .map(|(key, value)| ((*key).to_owned(), Value::from(*value)))
                    .collect();
                context.add_variable_from_value("attributes", attributes);
            }
        }
    }

    fn sample() -> TestBindings {
        TestBindings::new(
            "checkout payment",
            vec![("myapp.checkout.stage", "payment")],
        )
    }

    fn evaluate(source: &str) -> Result<bool, Error> {
        Expression::compile(source)
            .expect("it compiles")
            .evaluate(&sample())
    }

    #[test]
    fn an_expression_that_does_not_parse_is_rejected() {
        let error = Expression::compile("attributes[").expect_err("it does not compile");
        assert!(matches!(error, Error::CompileFailed { .. }), "{error}");
    }

    #[test]
    fn a_literal_pattern_that_is_not_a_valid_regex_is_rejected_at_compile_time() {
        // A lookbehind, which the `regex` crate does not support.
        let error = Expression::compile(r#"name.matches("^(?<=cart)payment$")"#)
            .expect_err("it does not compile");
        assert!(matches!(error, Error::BadPattern { .. }), "{error}");
    }

    /// A pattern built at run time is not visible when compiling.
    #[test]
    fn a_computed_pattern_still_errors_at_evaluation() {
        let error = evaluate(r#"name.matches("^(?<=" + "cart)payment$")"#).expect_err("it errors");
        assert!(matches!(error, Error::EvalFailed { .. }), "{error}");
    }

    #[test]
    fn a_valid_pattern_matches_and_does_not_match() {
        assert!(evaluate(r#"name.matches("^checkout ")"#).expect("it evaluates"));
        assert!(!evaluate(r#"name.matches("^payment ")"#).expect("it evaluates"));
    }

    #[test]
    fn an_expression_that_is_not_a_bool_is_rejected() {
        let error = evaluate(r#"attributes["myapp.checkout.stage"]"#).expect_err("not a bool");
        assert!(matches!(error, Error::NotBoolean { .. }), "{error}");
    }

    #[test]
    fn a_guarded_read_comes_out_false() {
        assert!(
            !evaluate(r#""absent" in attributes && attributes["absent"] == "x""#)
                .expect("it evaluates")
        );
    }

    /// Error absorption in `&&` is commutative.
    #[test]
    fn the_guard_works_from_either_side() {
        assert!(
            !evaluate(r#"attributes["absent"] == "x" && "absent" in attributes"#)
                .expect("it evaluates")
        );
    }

    /// Absorption only applies while the other side is false.
    #[test]
    fn a_guard_on_another_key_still_errors() {
        let error =
            evaluate(r#""myapp.checkout.stage" in attributes && attributes["absent"] == "x""#)
                .expect_err("it errors");
        assert!(matches!(error, Error::EvalFailed { .. }), "{error}");
    }

    /// An unbound variable errors in the same way as an absent key.
    #[test]
    fn an_unbound_variable_errors() {
        let error = evaluate(r#"unit == "s""#).expect_err("it errors");
        assert!(matches!(error, Error::EvalFailed { .. }), "{error}");
    }

    #[test]
    fn the_variables_an_expression_reads_are_known_after_compiling() {
        let expression = Expression::compile(r#"name.startsWith("checkout")"#).expect("compiles");
        assert!(expression.referenced().wants("name"));
        assert!(!expression.referenced().wants("attributes"));
        assert_eq!(
            expression.referenced().variables().collect::<Vec<_>>(),
            ["name"]
        );
    }

    #[test]
    fn a_comprehension_binds_its_own_loop_variable() {
        let expression = Expression::compile(r#"attributes.exists(k, k.startsWith("myapp."))"#)
            .expect("compiles");
        assert_eq!(
            expression.referenced().variables().collect::<Vec<_>>(),
            ["attributes"]
        );
        assert!(expression.evaluate(&sample()).expect("it evaluates"));
    }

    #[test]
    fn a_name_used_free_outside_a_comprehension_is_still_referenced() {
        let expression =
            Expression::compile(r#"attributes.exists(name, name != "") && name != """#)
                .expect("compiles");
        assert!(expression.referenced().wants("name"));
    }

    #[test]
    fn a_scope_binds_once_for_every_expression_in_it() {
        let expressions: Vec<Expression> = [
            r#"name.startsWith("checkout")"#,
            r#"attributes["myapp.checkout.stage"] == "payment""#,
            r#"name.endsWith("payment")"#,
        ]
        .iter()
        .map(|source| Expression::compile(source).expect("compiles"))
        .collect();
        let referenced = Referenced::union(expressions.iter());
        assert_eq!(
            referenced.variables().collect::<Vec<_>>(),
            ["attributes", "name"]
        );

        let bindings = sample();
        let scope = Scope::new(&referenced, &bindings);
        for expression in &expressions {
            assert!(expression.evaluate_in(&scope).expect("it evaluates"));
        }
        assert_eq!(*bindings.bound.borrow(), ["name", "attributes"]);
    }

    /// An expression that only reads `name` does not build the attribute map.
    #[test]
    fn only_the_referenced_variables_are_bound() {
        let expression = Expression::compile(r#"name.startsWith("checkout")"#).expect("compiles");
        let bindings = sample();
        assert!(expression.evaluate(&bindings).expect("it evaluates"));
        assert_eq!(*bindings.bound.borrow(), ["name"]);
    }
}
