// SPDX-License-Identifier: Apache-2.0

//! Utility functions to index and render resources.

use crate::search::DocFields;
use tantivy::{doc, IndexWriter};
use weaver_schema::attribute::Attribute;
use weaver_schema::TelemetrySchema;

/// Build index for resources.
pub fn index(schema: &TelemetrySchema, fields: &DocFields, index_writer: &mut IndexWriter) {
    if let Some(resource) = schema.resource() {
        for attr in resource.attributes() {
            if let Attribute::Id {
                id: the_id,
                brief: the_brief,
                note: the_note,
                tag: the_tag,
                ..
            } = attr
            {
                index_writer
                    .add_document(doc!(
                        fields.path => format!("schema/resource/attr/{}", the_id),
                        fields.brief => the_brief.as_str(),
                        fields.note => the_note.as_str(),
                        fields.tag => the_tag.as_ref().unwrap_or(&"".to_string()).as_str(),
                    ))
                    .expect("Failed to add document");
            }
        }
    }
}
