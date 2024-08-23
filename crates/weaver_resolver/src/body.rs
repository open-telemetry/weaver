// SPDX-License-Identifier: Apache-2.0

//! Functions to resolve a semantic convention body.

use weaver_resolved_schema::{
    body::{Body, BodyField},
    error::Error,
};
use weaver_semconv::body::BodySpec;

/// Resolve a `Body` specification into a resolved `Body`.
pub fn resolve_body_spec(body: &BodySpec) -> Result<Option<Body>, Error> {
    match body {
        BodySpec::Fields { fields } => {
            let mut body_fields = Vec::new();
            for field in fields.iter() {
                body_fields.push(BodyField {
                    name: field.id.clone(),
                    r#type: field.r#type.clone(),
                    brief: field.brief.clone().unwrap_or_else(|| "".to_owned()),
                    examples: field.examples.clone(),
                    requirement_level: field.requirement_level.clone(),
                    note: field.note.clone(),
                    stability: field.stability.clone(),
                    deprecated: field.deprecated.clone(),
                });
            }
            Ok(Some(Body {
                fields: Some(body_fields),
                // value: None,             // Not yet implemented
            }))
        }
        BodySpec::Value { value: _ } => {
            // Add as a placeholder for now of where to resolve the value.
            Err(Error::NotImplemented {
                message: "Value type for body is not currently implemented.".to_owned(),
            })
        }
    }
}
