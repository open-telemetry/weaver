// SPDX-License-Identifier: Apache-2.0

//! Resolves or converts tags into their resolved form.

use weaver_schema::tags::Tags;

/// Converts tags into their resolved form.
#[allow(dead_code)] // ToDo Remove this once we have tags in the resolved schema
pub fn semconv_to_resolved_tags(tags: &Option<Tags>) -> Option<weaver_resolved_schema::tags::Tags> {
    tags.as_ref()
        .map(|tags| weaver_resolved_schema::tags::Tags {
            tags: tags.tags.clone(),
        })
}
