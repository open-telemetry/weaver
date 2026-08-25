// SPDX-License-Identifier: Apache-2.0

//! Compiled expressions.

use std::collections::BTreeSet;

use cel::{Context, Program, Value};

use crate::{Bindings, Error};

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
        let referenced = Referenced {
            variables: program
                .references()
                .variables()
                .into_iter()
                .map(str::to_owned)
                .collect(),
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
        let mut context = Context::default();
        bindings.bind(&self.referenced, &mut context);
        let value = self
            .program
            .execute(&context)
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

    /// An expression that only reads `name` does not build the attribute map.
    #[test]
    fn only_the_referenced_variables_are_bound() {
        let expression = Expression::compile(r#"name.startsWith("checkout")"#).expect("compiles");
        let bindings = sample();
        assert!(expression.evaluate(&bindings).expect("it evaluates"));
        assert_eq!(*bindings.bound.borrow(), ["name"]);
    }
}
