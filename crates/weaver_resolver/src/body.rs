// SPDX-License-Identifier: Apache-2.0

//! Functions to resolve a semantic convention body.

use weaver_resolved_schema::{
    body::{Body, BodyField},
    error::{handle_errors, Error},
};
use weaver_semconv::body::{BodyFieldSpec, BodySpec};

use crate::attribute::AttributeCatalog;

/// Resolve a body specification into a resolved body.
pub fn resolve_body_spec(
    body: &BodySpec,
    attr_catalog: &mut AttributeCatalog,
) -> Result<Option<Body>, Error> {
    match body {
        BodySpec::Fields { fields } => {
            let mut errors = vec![];
            let mut body_fields = Vec::new();
            for field in fields.iter() {
                match resolve_body_field_spec(field, attr_catalog) {
                    Ok(r) => body_fields.push(r),
                    Err(e) => errors.push(e),
                }
            }
            handle_errors(errors)?;
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

/// Resolve a body field specification into a resolved body field.
pub fn resolve_body_field_spec(
    field: &BodyFieldSpec,
    attr_catalog: &mut AttributeCatalog,
) -> Result<BodyField, Error> {
    match field {
        BodyFieldSpec::Ref {
            r#ref,
            alias,
            brief,
            examples,
            requirement_level,
            note,
            stability,
            deprecated,
        } => {
            let attr_ref = attr_catalog.get_attribute_ref(r#ref)?;

            let root_attr = attr_ref.attr.attribute.clone();
            Ok(BodyField {
                name: field.id(),
                r#attr: Some(attr_ref.attr_ref),
                alias: alias.clone(),
                r#type: root_attr.r#type.clone(),
                brief: {
                    match brief {
                        Some(brief) => brief.clone(),
                        None => root_attr.brief.to_owned(),
                    }
                },
                examples: {
                    match examples {
                        Some(examples) => Some(examples.to_owned()),
                        None => root_attr.examples.to_owned(),
                    }
                },
                requirement_level: {
                    match requirement_level {
                        Some(level) => level.clone(),
                        None => root_attr.requirement_level.to_owned(),
                    }
                },
                note: {
                    match note {
                        Some(note) => note.clone(),
                        None => root_attr.note.to_owned(),
                    }
                },
                stability: {
                    match stability {
                        Some(stability) => Some(stability.to_owned()),
                        None => root_attr.stability.to_owned(),
                    }
                },
                deprecated: {
                    match deprecated {
                        Some(deprecated) => Some(deprecated.to_owned()),
                        None => root_attr.deprecated.to_owned(),
                    }
                },
            })
        }
        BodyFieldSpec::Id {
            id,
            r#type,
            brief,
            examples,
            requirement_level,
            note,
            stability,
            deprecated,
        } => Ok(BodyField {
            name: id.clone(),
            r#attr: None,
            alias: None,
            r#type: r#type.clone(),
            brief: {
                match brief {
                    Some(brief) => brief.clone(),
                    None => "".to_owned(),
                }
            },
            examples: examples.clone(),
            requirement_level: requirement_level.clone(),
            note: note.clone(),
            stability: stability.clone(),
            deprecated: deprecated.clone(),
        }),
    }
}
