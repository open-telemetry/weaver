# Weaver CEL

A thin wrapper over the [`cel`](https://crates.io/crates/cel) crate. It
compiles and evaluates [CEL](https://cel.dev) expressions that return a bool.

The crate compiles, inspects and evaluates expressions. It has no telemetry
types and no weaver dependencies. The crate that owns the data implements
`Bindings` and decides the variable names. `weaver_live_check` uses it for
`[[live-check.matchers]]`, where a `when` expression selects the samples that
a matcher applies to.

## API

| Item | Purpose |
| --- | --- |
| `Expression::compile` | Parses the source text once. |
| `Expression::evaluate` | Runs the expression against a set of bindings and returns a bool. |
| `Expression::referenced` | The variables the expression reads. Known after compilation. |
| `Bindings` | Implemented by the caller to supply variable values. |
| `Referenced::union` | The variables that a set of expressions read, so they can be bound in one pass. |
| `Scope` / `Expression::evaluate_in` | Binds once, then evaluates several expressions against the same bindings. |
| `Error` | `CompileFailed`, `BadPattern`, `EvalFailed` or `NotBoolean`. Each carries the source text. |

`Context` and `Value` are re-exported so that implementors of `Bindings` do not
need a direct `cel` dependency.

## Example

```rust
use std::collections::HashMap;
use weaver_cel::{Bindings, Context, Expression, Referenced, Value};

struct Span {
    name: String,
    attributes: HashMap<String, Value>,
}

impl Bindings for Span {
    fn bind(&self, referenced: &Referenced, context: &mut Context<'_>) {
        if referenced.wants("name") {
            context.add_variable_from_value("name", self.name.as_str());
        }
        if referenced.wants("attributes") {
            context.add_variable_from_value("attributes", self.attributes.clone());
        }
    }
}

fn main() -> Result<(), weaver_cel::Error> {
    let span = Span {
        name: "checkout payment".to_owned(),
        attributes: HashMap::from([("myapp.checkout.stage".to_owned(), Value::from("payment"))]),
    };

    let expression = Expression::compile(r#"name.startsWith("checkout")"#)?;
    assert!(expression.evaluate(&span)?);
    Ok(())
}
```

## Notes

`bind` receives the `Referenced` set so that it can skip work. An expression
that only reads `name` never builds the attribute map. To evaluate several
expressions against one sample, use a `Scope`. It binds the union of the
variables they read once.

A read of an absent map key, or of a variable that is not bound, is an error.
It does not evaluate to `false`. Guard the read with `in`, as in
`"a.b" in attributes && attributes["a.b"] == "x"`. CEL evaluates this to
`false` whichever side the guard is on.

An expression that returns a value other than a bool is a `NotBoolean` error at
evaluation time, not at compile time. The `cel` crate has no type checker, so a
call to an unknown function, or a call with the wrong number of arguments, is
also a runtime error.
