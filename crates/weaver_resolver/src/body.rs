// SPDX-License-Identifier: Apache-2.0

//! Functions to resolve a semantic convention body.

use weaver_resolved_schema::{
    body::{Body, BodyField},
    error::Error,
};
use weaver_semconv::body::{BodySpec, BodyType};

/// Resolve a `Body` specification into a resolved `Body`.
pub fn resolve_body_spec(body: &BodySpec) -> Result<Option<Body>, Error> {
    match body {
        BodySpec::Fields {
            r#type: BodyType::Map,
            brief,
            note,
            stability,
            examples,
            fields,
            ..
        } => {
            let mut body_fields = Vec::new();
            for field in fields.iter() {
                body_fields.push(BodyField {
                    name: field.id.clone(),
                    r#type: field.r#type.clone(),
                    brief: field.brief.clone(),
                    examples: field.examples.clone(),
                    requirement_level: field.requirement_level.clone(),
                    note: field.note.clone(),
                    stability: field.stability.clone(),
                    deprecated: field.deprecated.clone(),
                });
            }
            Ok(Some(Body {
                r#type: BodyType::Map,
                brief: brief.clone(),
                note: note.clone(),
                stability: stability.clone(),
                examples: examples.clone(),
                fields: Some(body_fields),
            }))
        }
        BodySpec::String {
            r#type: BodyType::String,
            brief,
            note,
            stability,
            examples,
        } => {
            // string types must have a brief and examples
            if brief.is_empty() || examples.is_none() {
                return Err(Error::InvalidBody { body: body.clone() });
            }
            Ok(Some(Body {
                r#type: BodyType::String,
                brief: brief.clone(),
                note: note.clone(),
                stability: stability.clone(),
                examples: examples.clone(),
                fields: None,
            }))
        }
        _ => Err(Error::InvalidBody { body: body.clone() }),
    }
}
