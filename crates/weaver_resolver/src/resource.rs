// SPDX-License-Identifier: Apache-2.0

//! Resolve resource

use crate::attribute::resolve_attributes;
use crate::Error;
use weaver_schema::schema_spec::SchemaSpec;
use weaver_semconv::registry::SemConvRegistry;
use weaver_version::VersionChanges;

/// Resolves resource attributes.
pub fn resolve_resource(
    schema: &mut SchemaSpec,
    semconv_registry: &SemConvRegistry,
    version_changes: &VersionChanges,
) -> Result<(), Error> {
    // Resolve resource attributes
    if let Some(res) = schema.resource.as_mut() {
        res.attributes = resolve_attributes(
            res.attributes.as_ref(),
            semconv_registry,
            version_changes.log_attribute_changes(),
        )?;
    }
    Ok(())
}
