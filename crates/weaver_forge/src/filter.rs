// SPDX-License-Identifier: Apache-2.0

//! Filter JSON values using a simple expression language.

use core::fmt;
use std::fmt::Debug;
use jaq_interpret::{Ctx, FilterT, RcIter, Val};
use serde::de;
use crate::Error;

/// A filter that can be applied to a JSON value.
pub struct Filter {
    filter_expr: String,
    filter: jaq_interpret::Filter,
}

impl Filter {
    /// Create a new filter from a string expression or return an error if the
    /// expression is invalid.
    pub fn try_new(filter_expr: &str) -> Result<Self, Error> {
        let vars = Vec::new();
        let mut ctx = jaq_interpret::ParseCtx::new(vars);
        ctx.insert_natives(jaq_core::core());
        ctx.insert_defs(jaq_std::std());

        let (parsed_expr, errs) = jaq_parse::parse(filter_expr, jaq_parse::main());

        // If there are any errors, return them
        if !errs.is_empty() {
            return Err(Error::CompoundError(errs.into_iter().map(|e| Error::FilterError {
                filter: filter_expr.to_string(),
                error: e.to_string()
            }).collect()));
        }

        let parsed_expr = parsed_expr.ok_or_else(|| Error::FilterError {
            filter: filter_expr.to_string(),
            error: "No parsed expression".to_string()
        })?;

        Ok(Self {
            filter_expr: filter_expr.to_string(),
            filter: ctx.compile(parsed_expr),
        })
    }

    /// Apply the filter to a JSON value and return the result as a JSON value.
    pub fn apply(&self, json: serde_json::Value) -> Result<serde_json::Value, Error> {
        let inputs = RcIter::new(core::iter::empty());
        let mut filter_result = self.filter.run((Ctx::new([], &inputs), Val::from(json)));
        let mut errs = Vec::new();
        let mut values = Vec::new();

        while let Some(r) = filter_result.next() {
            match r {
                Ok(v) => values.push(serde_json::Value::from(v)),
                Err(e) => errs.push(e),
            }
        }

        if values.len() == 1 {
            return Ok(values.pop().unwrap());
        }

        Ok(serde_json::Value::Array(values))
    }
}

impl Debug for Filter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Filter({})", self.filter_expr)
    }
}

struct FilterVisitor;

impl<'de> de::Visitor<'de> for FilterVisitor {
    type Value = Filter;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a filter string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
    {
        Filter::try_new(value).map_err(E::custom)
    }
}

impl<'de> de::Deserialize<'de> for Filter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
    {
        deserializer.deserialize_str(FilterVisitor)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use crate::filter::Filter;

    #[test]
    fn test_jaq() -> Result<(), crate::Error> {
        let filter = Filter::try_new(".b").unwrap();
        let json = json!({"a": 1, "b": {"c": 1, "d": 2}});
        assert_eq!(filter.apply(json)?, json!([{"c": 1, "d": 2}]));
        Ok(())
    }
}