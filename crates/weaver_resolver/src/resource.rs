// SPDX-License-Identifier: Apache-2.0

//! Resolve resource

use crate::attribute::resolve_attributes;
use crate::Error;
use weaver_schema::schema_spec::SchemaSpec;
use weaver_semconv::SemConvSpecs;
use weaver_version::VersionChanges;

/// Resolves resource attributes.
pub fn resolve_resource(
    schema: &mut SchemaSpec,
    sem_conv_catalog: &SemConvSpecs,
    version_changes: &VersionChanges,
) -> Result<(), Error> {
    // Resolve resource attributes
    if let Some(res) = schema.resource.as_mut() {
        res.attributes = resolve_attributes(
            res.attributes.as_ref(),
            sem_conv_catalog,
            version_changes.log_attribute_changes(),
        )?;
    }
    Ok(())
}
