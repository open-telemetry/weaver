// SPDX-License-Identifier: Apache-2.0

//! Resolve events

use crate::attribute::resolve_attributes;
use crate::Error;
use weaver_schema::schema_spec::SchemaSpec;
use weaver_semconv::registry::SemConvRegistry;
use weaver_version::VersionChanges;

/// Resolves resource events and their attributes.
pub fn resolve_events(
    schema: &mut SchemaSpec,
    semconv_registry: &SemConvRegistry,
    version_changes: &VersionChanges,
) -> Result<(), Error> {
    if let Some(events) = schema.resource_events.as_mut() {
        events.attributes = resolve_attributes(
            events.attributes.as_ref(),
            semconv_registry,
            version_changes.log_attribute_changes(),
        )?;
        for event in events.events.iter_mut() {
            event.attributes = resolve_attributes(
                event.attributes.as_ref(),
                semconv_registry,
                version_changes.log_attribute_changes(),
            )?;
        }
    }
    Ok(())
}
