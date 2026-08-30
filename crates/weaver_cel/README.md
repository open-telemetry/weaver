# Weaver CEL

A thin wrapper over the [`cel`](https://crates.io/crates/cel) crate for
compiling and evaluating [CEL](https://cel.dev) expressions that return a bool.

Compilation, inspection and evaluation only: no telemetry types and no weaver
dependencies. The crate that owns the data implements `Bindings` and decides
the variable names. `weaver_live_check` uses it for `[[live-check.matchers]]`,
where a `when` expression selects the samples a matcher applies to.

## API

| Item | Purpose |
| --- | --- |
| `Expression::compile` | Parses source text once. |
| `Expression::evaluate` | Runs it against a set of bindings, returning a bool. |
| `Expression::referenced` | The variables the expression reads, known after compiling. |
| `Bindings` | Implemented by the caller to supply variable values. |
| `Error` | `CompileFailed`, `EvalFailed` or `NotBoolean`, each carrying the source text. |

`Context` and `Value` are re-exported so implementors of `Bindings` need no
direct `cel` dependency.

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
            context.add_variable_from_value("name", self.name.clone());
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

`bind` is passed the `Referenced` set so it can skip work: an expression that
only reads `name` never builds the attribute map.

Reading an absent map key or an unbound variable is an error, not `false`.
Guard with `in`, as in `"a.b" in attributes && attributes["a.b"] == "x"`, which
CEL evaluates to `false` from either side.

An expression that returns anything other than a bool is a `NotBoolean` error
at evaluation, not at compile time. The `cel` crate parses without a
type-checker, so a call to an unknown function or with the wrong arity is also
a runtime error.
