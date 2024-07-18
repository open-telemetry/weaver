// SPDX-License-Identifier: Apache-2.0

//! Filter JSON values using a simple expression language.

use crate::error::Error;
use core::fmt;
use jaq_interpret::{Ctx, FilterT, RcIter, Val};
use std::fmt::Debug;
use jaq_syn::Def;

/// A filter that can be applied to a JSON value.
pub struct Filter {
    filter_expr: String,
    filter: jaq_interpret::Filter,
}

impl Filter {
    /// Create a new filter from a string expression or return an error if the
    /// expression is invalid.
    /// The vars parameter is a list of variable names that can be used in the
    /// filter expression.
    pub fn try_new(
        filter_expr: &str,
        vars: Vec<String>,
        defs: Vec<Def>
    ) -> Result<Self, Error> {
        let mut ctx = jaq_interpret::ParseCtx::new(vars);
        ctx.insert_natives(jaq_core::core());
        ctx.insert_defs(jaq_std::std());
        ctx.insert_defs(defs);

        let (parsed_expr, errs) = jaq_parse::parse(filter_expr, jaq_parse::main());

        // If there are any errors, return them
        if !errs.is_empty() {
            return Err(Error::CompoundError(
                errs.into_iter()
                    .map(|e| Error::FilterError {
                        filter: filter_expr.to_owned(),
                        error: e.to_string(),
                    })
                    .collect(),
            ));
        }

        let parsed_expr = parsed_expr.ok_or_else(|| Error::FilterError {
            filter: filter_expr.to_owned(),
            error: "No parsed expression".to_owned(),
        })?;

        Ok(Self {
            filter_expr: filter_expr.to_owned(),
            filter: ctx.compile(parsed_expr),
        })
    }

    /// Apply the filter to a JSON value and return the result as a JSON value.
    pub fn apply(
        &self,
        ctx: serde_json::Value,
        jq_ctx: Vec<Val>,
    ) -> Result<serde_json::Value, Error> {
        let inputs = RcIter::new(core::iter::empty());
        let filter_result = self.filter.run((Ctx::new(jq_ctx, &inputs), Val::from(ctx)));
        let mut errs = Vec::new();
        let mut values = Vec::new();

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
}

impl Debug for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Filter({})", self.filter_expr)
    }
}

#[cfg(test)]
mod tests {
    use jaq_interpret::Val;

    #[test]
    fn test_filter() {
        let filter = super::Filter::try_new("true", Vec::new(), Vec::new()).unwrap();
        let result = filter.apply(serde_json::json!({}), Vec::new()).unwrap();
        assert_eq!(result, serde_json::json!(true));

        let filter = super::Filter::try_new(".", Vec::new(), Vec::new()).unwrap();
        let result = filter.apply(serde_json::json!({}), Vec::new()).unwrap();
        assert_eq!(result, serde_json::Value::Object(serde_json::Map::new()));

        let filter = super::Filter::try_new(".", Vec::new(), Vec::new()).unwrap();
        let result = filter
            .apply(
                serde_json::json!({
                    "a": 1,
                    "b": 2,
                }),
                Vec::new(),
            )
            .unwrap();
        assert_eq!(
            result,
            serde_json::json!({
                "a": 1,
                "b": 2,
            })
        );

        let filter = super::Filter::try_new(".key1", Vec::new(), Vec::new()).unwrap();
        let result = filter
            .apply(
                serde_json::json!({
                    "key1": 1,
                    "key2": 2,
                }),
                Vec::new(),
            )
            .unwrap();
        assert_eq!(result, serde_json::json!(1));

        let filter = super::Filter::try_new(".[\"key1\"]", Vec::new(), Vec::new()).unwrap();
        let result = filter
            .apply(
                serde_json::json!({
                    "key1": 1,
                    "key2": 2,
                }),
                Vec::new(),
            )
            .unwrap();
        assert_eq!(result, serde_json::json!(1));

        let vars = vec!["key".to_owned()];
        let ctx = vec![Val::from(serde_json::Value::String("key1".to_owned()))];
        let filter = super::Filter::try_new(".[$key]", vars, Vec::new()).unwrap();
        let result = filter
            .apply(
                serde_json::json!({
                    "key1": 1,
                    "key2": 2,
                }),
                ctx,
            )
            .unwrap();
        assert_eq!(result, serde_json::json!(1));

        let jq_filter = r#"
if $incubating then
  .
else
  null
end"#;
        let input = serde_json::json!({
            "key1": 1,
            "key2": 2,
        });
        let vars = vec!["incubating".to_owned()];
        // When incubating is true, the entire input is returned.
        let ctx = vec![Val::from(serde_json::Value::Bool(true))];
        let filter = super::Filter::try_new(jq_filter, vars.clone(), Vec::new()).unwrap();
        let result = filter.apply(input.clone(), ctx).unwrap();
        assert_eq!(result, input);

        // When incubating = false the filter should return an empty array
        let ctx = vec![Val::from(serde_json::Value::Bool(false))];
        let filter = super::Filter::try_new(jq_filter, vars, Vec::new()).unwrap();
        let result = filter.apply(input.clone(), ctx).unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }
}
