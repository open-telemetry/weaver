// SPDX-License-Identifier: Apache-2.0

//! Filter JSON values using a simple expression language.

use crate::error::Error;
use core::fmt;
use std::{collections::BTreeMap, fmt::Debug};

/// A filter that can be applied to a JSON value.
pub struct Filter {
    filter_expr: String,
}

impl Filter {
    /// Create a new filter from a string expression or return an error if the
    /// expression is invalid.
    /// The vars parameter is a list of variable names that can be used in the
    /// filter expression.
    pub fn new(filter_expr: &str) -> Self {
        Self {
            filter_expr: filter_expr.to_owned(),
        }
    }

    /// Apply the filter to a JSON value and return the result as a JSON value.
    pub fn apply(
        &self,
        ctx: serde_json::Value,
        values: &BTreeMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, Error> {
        crate::jq::execute_jq(&ctx, &self.filter_expr, values)
    }
}

impl Debug for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Filter({})", self.filter_expr)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    #[test]
    fn test_filter() {
        let filter = super::Filter::new("true");
        let result = filter
            .apply(serde_json::json!({}), &BTreeMap::new())
            .unwrap();
        assert_eq!(result, serde_json::json!(true));

        let filter = super::Filter::new(".");
        let result = filter
            .apply(serde_json::json!({}), &BTreeMap::new())
            .unwrap();
        assert_eq!(result, serde_json::Value::Object(serde_json::Map::new()));

        let filter = super::Filter::new(".");
        let result = filter
            .apply(
                serde_json::json!({
                    "a": 1,
                    "b": 2,
                }),
                &BTreeMap::new(),
            )
            .unwrap();
        assert_eq!(
            result,
            serde_json::json!({
                "a": 1,
                "b": 2,
            })
        );

        let filter = super::Filter::new(".key1");
        let result = filter
            .apply(
                serde_json::json!({
                    "key1": 1,
                    "key2": 2,
                }),
                &BTreeMap::new(),
            )
            .unwrap();
        assert_eq!(result, serde_json::json!(1));

        let filter = super::Filter::new(".[\"key1\"]");
        let result = filter
            .apply(
                serde_json::json!({
                    "key1": 1,
                    "key2": 2,
                }),
                &BTreeMap::new(),
            )
            .unwrap();
        assert_eq!(result, serde_json::json!(1));

        let mut vars = BTreeMap::new();
        let _ = vars.insert(
            "key".to_owned(),
            serde_json::Value::String("key1".to_owned()),
        );
        let filter = super::Filter::new(".[$key]");
        let result = filter
            .apply(
                serde_json::json!({
                    "key1": 1,
                    "key2": 2,
                }),
                &vars,
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
        // When incubating is true, the entire input is returned.
        let mut ctx = BTreeMap::new();
        let _ = ctx.insert("incubating".to_owned(), serde_json::Value::Bool(true));
        let filter = super::Filter::new(jq_filter);
        let result = filter.apply(input.clone(), &ctx).unwrap();
        assert_eq!(result, input);

        // When incubating = false the filter should return an empty array
        let _ = ctx.insert("incubating".to_owned(), serde_json::Value::Bool(false));
        let filter = super::Filter::new(jq_filter);
        let result = filter.apply(input.clone(), &ctx).unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }
}
