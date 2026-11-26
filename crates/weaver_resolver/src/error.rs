// SPDX-License-Identifier: Apache-2.0

//! Errors emitted during telemetry schema resolution.

use miette::Diagnostic;
use serde::Serialize;
use std::path::PathBuf;
use weaver_common::diagnostic::{DiagnosticMessage, DiagnosticMessages};
use weaver_common::error::{format_errors, WeaverError};
use weaver_common::{log_error, serialize_normalized_path};
use weaver_semconv::provenance::Provenance;
use weaver_semconv::schema_url::SchemaUrl;
pub use weaver_semconv::AttributeRole;

use crate::graph::loader::SourceFormat;

/// Formats an optional provenance suffix for error display messages.
fn format_provenance(prov: &Option<Box<Provenance>>) -> String {
    match prov {
        Some(p) => format!("\nProvenance: {p}"),
        None => String::new(),
    }
}

/// An error that can occur while resolving a telemetry schema.
#[derive(thiserror::Error, Debug, Clone, Serialize, Diagnostic)]
#[must_use]
#[non_exhaustive]
pub enum Error {
    /// Failed to resolve a semantic convention definition.
    #[error(transparent)]
    #[diagnostic(transparent)]
    FailToResolveDefinition(weaver_semconv::Error),

    /// Failed to access a virtual directory path.
    #[error(transparent)]
    #[diagnostic(transparent)]
    VirtualDirectoryError(#[from] weaver_common::Error),

    /// Circular dependency between registries.
    #[error("Circular dependency detected: registry '{registry_name}' depends on itself through the chain: {chain}")]
    CircularDependency {
        /// The registry that depends on itself.
        registry_name: String,

        /// A string representing the dependency chain.
        chain: String,
    },

    /// Dependency chain exceeded the maximum allowed depth.
    #[error("Maximum dependency depth reached for registry `{registry_name}`. Cannot load further dependencies.")]
    MaximumDependencyDepth {
        /// The registry which has too many dependencies.
        registry_name: String,
    },

    /// Failed to resolve the schema URL for a registry.
    #[error("Schema URL is missing in the manifest and cannot be constructed from the registry name and version.")]
    FailToResolveSchemaUrl {},

    /// The schema is missing from the resolved schema cache.
    #[error("Schema `{schema_url}` is not present in the resolved schema cache")]
    #[diagnostic(
        code(weaver_resolver::schema_not_in_cache),
        help("The schema was not loaded during resolution and is unavailable in the snapshot.")
    )]
    SchemaNotInCache {
        /// The schema URL that was not found in cache.
        schema_url: SchemaUrl,
    },

    /// A registry is declared more than once and at least one declaration has no version.
    #[error("Registry '{name}' is declared by more than one dependency, but at least one declaration is unversioned")]
    #[diagnostic(help(
        "Use a `schema_url` with a semantic version, e.g. https://example.com/{name}/1.0.0."
    ))]
    UnversionedDependencyConflict {
        /// The registry declared more than once.
        name: String,
    },

    /// An invalid URL.
    #[error("Invalid URL `{url:?}`, error: {error:?})")]
    #[diagnostic(help("Check the URL and try again."))]
    InvalidUrl {
        /// The invalid URL.
        url: String,
        /// The error that occurred.
        error: String,
    },

    /// Failed to resolve a set of attributes.
    #[error("Failed to resolve a set of attributes {ids:?}: {error}")]
    FailToResolveAttributes {
        /// The ids of the attributes.
        ids: Vec<String>,
        /// The error that occurred.
        error: String,
    },

    /// Failed to resolve a metric.
    #[error("Failed to resolve the metric '{ref}'")]
    FailToResolveMetric {
        /// The reference to the metric.
        r#ref: String,
    },

    /// Metric attributes are incompatible within the metric group.
    #[error("Metric attributes are incompatible within the metric group '{metric_group_ref}' for metric '{metric_ref}' (error: {error})")]
    IncompatibleMetricAttributes {
        /// The metric group reference.
        metric_group_ref: String,
        /// The reference to the metric.
        metric_ref: String,
        /// The error that occurred.
        error: String,
    },

    /// A generic conversion error.
    #[error("Conversion error: {message}")]
    ConversionError {
        /// The error that occurred.
        message: String,
    },

    /// An unresolved attribute reference.
    #[error("The following attribute reference is not resolved for the group '{group_id}'.\nAttribute reference: {attribute_ref}{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(help(
        "Define the attribute in this registry, or import it from a dependency via `imports:`."
    ))]
    UnresolvedAttributeRef {
        /// The id of the group containing the attribute reference.
        group_id: String,
        /// The unresolved attribute reference.
        attribute_ref: String,
        /// The provenance of the reference (URL or path).
        provenance: Option<Box<Provenance>>,
    },

    /// An `entity_associations` entry references an undefined entity.
    #[error("The following entity association is not resolved for the group '{group_id}'.\nEntity association: {entity_type}{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(help(
        "Check the spelling. Define the entity in this registry, or depend on a registry that defines it."
    ))]
    UnresolvedEntityAssociation {
        /// The id of the group that holds the association.
        group_id: String,
        /// The undefined entity type.
        entity_type: String,
        /// The provenance of the group (URL or path).
        provenance: Option<Box<Provenance>>,
    },

    /// An `entity_associations` entry matches entities declared by multiple dependencies.
    #[error("The entity association '{entity_type}' of group '{group_id}' is ambiguous.\nDeclared by: {}", registries.join(", "))]
    #[diagnostic(help(
        "These registries declare unrelated entities under one name. Import the one you mean, or define the entity in this registry."
    ))]
    AmbiguousEntityAssociation {
        /// The id of the group that holds the association.
        group_id: String,
        /// The entity type declared by multiple registries.
        entity_type: String,
        /// The registries that declare an entity under that name.
        registries: Vec<String>,
    },

    /// An unresolved `extends` clause reference.
    #[error("The following `extends` clause reference is not resolved for the group '{group_id}'.\n`extends` clause reference: {extends_ref}{provenance}", provenance = format_provenance(.provenance))]
    UnresolvedExtendsRef {
        /// The id of the group containing the `extends` clause reference.
        group_id: String,
        /// The unresolved `extends` clause reference.
        extends_ref: String,
        /// The provenance of the reference (URL or path).
        provenance: Option<Box<Provenance>>,
    },

    /// An unresolved refinement parent reference.
    #[error("Refinement `{refinement_id}` refers to `{ref}` which is not resolved.{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(help(
        "Check that the parent `{r#ref}` is defined in this registry or in a declared dependency."
    ))]
    UnresolvedRefinementParent {
        /// The id of the refinement with the issue.
        refinement_id: String,
        /// The unresolved parent reference.
        r#ref: String,
        /// The provenance of the reference (URL or path).
        provenance: Option<Box<Provenance>>,
    },

    /// An unresolved `include` reference.
    #[error("The following `include` reference is not resolved for the group '{group_id}'.\n`include` reference: {include_ref}{provenance}", provenance = format_provenance(.provenance))]
    UnresolvedIncludeRef {
        /// The id of the group containing the `include` reference.
        group_id: String,
        /// The unresolved `include` reference.
        include_ref: String,
        /// The provenance of the reference (URL or path).
        provenance: Option<Box<Provenance>>,
    },

    /// An unresolved `ref_group` reference in a group or signal.
    #[error("The attribute group `{ref_group}` referenced by `{used_in}` is not resolved.{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(help(
        "`ref_group` resolves local `attribute_groups` only. If `{ref_group}` comes from a dependency, import it via `imports:` first."
    ))]
    UnresolvedRefGroup {
        /// The id of the group or signal containing the `ref_group` reference.
        used_in: String,
        /// The unresolved `ref_group` id.
        ref_group: String,
        /// The provenance of the reference.
        provenance: Option<Box<Provenance>>,
    },

    /// A `ref_group` references a signal instead of an `attribute_group`.
    #[error("`{used_in}` uses `ref_group: {ref_group}`, which references a `{signal_type}` signal rather than an `attribute_group`. Extract a shared `attribute_group` instead; referencing signals via `ref_group` is deprecated and will be removed in a future version.{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(severity(Warning))]
    DeprecatedSignalRefGroup {
        /// The signal, group, or refinement containing the `ref_group`.
        used_in: String,
        /// The `ref_group` target string (e.g. `span.gen_ai.client.inference`).
        ref_group: String,
        /// The kind of signal referenced (`span`, `metric`, or `event`).
        signal_type: String,
        /// Optional provenance of the referencing definition.
        #[serde(skip_serializing_if = "Option::is_none")]
        provenance: Option<Box<Provenance>>,
    },

    /// A cycle was detected in attribute group references.
    #[error("Cycle detected in attribute group references: {cycle}{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(help(
        "Break the cycle by removing one of the `ref_group` references in the chain."
    ))]
    AttributeGroupCycle {
        /// Description of the cycle.
        cycle: String,
        /// Optional provenance of the referencing group where the cycle was detected.
        #[serde(skip_serializing_if = "Option::is_none")]
        provenance: Option<Box<Provenance>>,
    },

    /// A reference targets an item marked `dependency_resolution.exclude: true`.
    /// Cross-registry references always fail; within the same registry, this
    /// only fires when the using item is not itself excluded.
    #[error("{type} '{id}' is marked as excluded from dependency resolution and cannot be used by '{used_in}'{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(help(
        "Remove `dependency_resolution.exclude: true` from `{id}`, or mark `{used_in}` as excluded as well."
    ))]
    ExcludedFromDependencyResolution {
        /// Id of the excluded attribute or group.
        id: String,
        /// Kind: "Attribute" or a `GroupType` name.
        r#type: String,
        /// Id of the group or signal using the excluded item, or `imports` for manifest imports.
        used_in: String,
        /// Optional provenance of the referencing group or signal.
        #[serde(skip_serializing_if = "Option::is_none")]
        provenance: Option<Box<Provenance>>,
    },

    /// An import pattern that named nothing in any dependency.
    #[error("The import `{pattern}` under `{signal}` matched nothing in any dependency")]
    #[diagnostic(severity(Warning))]
    #[diagnostic(help(
        "Check the spelling, and check that a dependency exports it. Imports are not transitive: a registry further down the chain is reachable only when the registry in between re-exports it."
    ))]
    UnmatchedImport {
        /// The pattern that matched nothing.
        pattern: String,
        /// The `imports` field the pattern is under.
        signal: String,
    },

    /// An invalid Schema path.
    #[error("Invalid Schema path: {path}")]
    InvalidSchemaPath {
        /// The schema path.
        #[serde(serialize_with = "serialize_normalized_path")]
        path: PathBuf,
    },

    /// A duplicate group id error.
    #[error("The group id `{group_id}` is declared multiple times in the following locations:\n{provenances:?}")]
    #[diagnostic(severity(Warning))]
    DuplicateGroupId {
        /// The group id.
        group_id: String,
        /// The provenances where this group is duplicated.
        provenances: Vec<Provenance>,
    },

    /// Multiple groups with distinct V1 IDs map to the same V2 signal ID.
    #[error("The groups {group_ids:?} all become `{signal_id}` in the v2 output, so one of them replaces the others:\n{provenances:?}")]
    #[diagnostic(severity(Warning))]
    #[diagnostic(help(
        "A v2 signal id drops the group-type prefix, so `entity.host` and `host` name one entity. Give each group an id that differs by more than that prefix."
    ))]
    CollidingV2SignalId {
        /// The id the groups share in the v2 output.
        signal_id: String,
        /// The ids of the groups that collide, in order.
        group_ids: Vec<String>,
        /// The provenances of those groups.
        provenances: Vec<Provenance>,
    },

    /// A duplicate group name warning.
    #[error("The group name `{group_name}` is declared multiple times in the following locations:\n{provenances:?}")]
    #[diagnostic(severity(Warning))]
    DuplicateGroupName {
        /// The group name.
        group_name: String,
        /// The provenances where this group is duplicated.
        provenances: Vec<Provenance>,
    },

    /// A duplicate metric name warning.
    #[error("The metric name `{metric_name}` is declared multiple times in the following locations:\n{provenances:?}")]
    #[diagnostic(severity(Warning))]
    DuplicateMetricName {
        /// The metric name.
        metric_name: String,
        /// The provenances where this metric name is duplicated.
        provenances: Vec<Provenance>,
    },

    /// A refinement targets a signal of a different type.
    #[error("Refinement `{refinement_id}` of type `{refinement_type}` cannot refine signal `{ref}` of type `{signal_type}`.")]
    InvalidRefinement {
        /// The id of the refinement with the issue.
        refinement_id: String,
        /// The reference being refined.
        r#ref: String,
        /// The type of the refinement.
        refinement_type: String,
        /// The type of the refined signal.
        signal_type: String,
    },

    /// A refinement cannot remove the requested attribute.
    #[error("Refinement `{refinement_id}` cannot unref attribute `{attribute_key}`: {reason}.{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(help(
        "Only optional, non-identifying attributes inherited from the parent signal may be removed via `unref:`."
    ))]
    InvalidAttributeUnref {
        /// The refinement requesting the removal.
        refinement_id: String,
        /// The attribute key to unreference.
        attribute_key: String,
        /// Why the removal is invalid.
        reason: String,
        /// The source of the refinement.
        provenance: Option<Box<Provenance>>,
    },

    /// Entity refinement changes identity.
    #[error("Entity refinement `{refinement_id}` changes the identity of `{ref}` by referencing `{attribute_id}` under `{role:?}`.{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(help(
        "A refinement must preserve the base entity's identity: keep base attributes under the same section (`identity` or `description`) as the base, and do not add new `identity` attributes."
    ))]
    EntityRefinementChangedIdentity {
        /// The id of the refinement with the issue.
        refinement_id: String,
        /// The entity being refined, as written in the refinement's `ref`.
        r#ref: String,
        /// The attribute whose identity role the refinement changes.
        attribute_id: String,
        /// The section the refinement lists the attribute under
        /// (`identity` or `description`).
        role: AttributeRole,
        /// The provenance of the refinement (URL or path).
        provenance: Option<Box<Provenance>>,
    },

    /// Entity refinement whose base entity has no identity attributes.
    #[error("Entity refinement `{refinement_id}` cannot refine `{ref}`: `{ref}` does not declare any `identity` attributes and is not valid under the v2 schema.{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(help(
        "The base entity must declare at least one `identity` attribute before it can be refined."
    ))]
    EntityRefinementWithoutIdentity {
        /// The id of the refinement with the issue.
        refinement_id: String,
        /// The entity being refined, as written in the refinement's `ref`.
        r#ref: String,
        /// The provenance of the refinement (URL or path).
        provenance: Option<Box<Provenance>>,
    },

    /// An entity has no identity attributes.
    #[error("Entity `{entity_type}` does not declare any `identity` attributes and is not valid under the v2 schema.{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(help("Add at least one `identity` attribute to entity `{entity_type}`."))]
    EntityMissingIdentity {
        /// Type of the entity.
        entity_type: String,
        /// Provenance.
        provenance: Option<Box<Provenance>>,
    },

    /// A V1 entity without identity attributes, resolved for V2, becomes a public attribute group.
    #[error("Entity `{group_id}` has no identity attributes and becomes a public attribute group in the v2 schema. It can't be used in entity associations, and entities extending it become standalone entities.{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(
        severity(Warning),
        help("Add at least one identifying attribute to keep it an entity.")
    )]
    EntityConvertedToAttributeGroup {
        /// The id of the entity group.
        group_id: String,
        /// Provenance.
        provenance: Option<Box<Provenance>>,
    },

    /// A duplicate attribute id error.
    #[error("The attribute id `{attribute_id}` is declared multiple times in the following groups:\n{group_ids:?}")]
    DuplicateAttributeId {
        /// The groups where this attribute is duplicated.
        group_ids: Vec<String>,
        /// The attribute id.
        attribute_id: String,
    },

    /// An attribute used in a span name template is not referenced on the span.
    #[error("Span `{span_id}` uses attribute `{attribute_key}` in its name templates, but the attribute is not declared or inherited on the span.{provenance}", provenance = format_provenance(.provenance))]
    #[diagnostic(help(
        "Add `{attribute_key}` to the span's `attributes` list, or remove it from the name template."
    ))]
    SpanNameAttributeNotOnSpan {
        /// The id/type of the span.
        span_id: String,
        /// The attribute key referenced in the template.
        attribute_key: String,
        /// The provenance of the span.
        provenance: Option<Box<Provenance>>,
    },

    /// Invalid import wildcard.
    #[error("Invalid import wildcard: {error:?}")]
    #[diagnostic(help(
        "Check the wildcard syntax supported here: https://crates.io/crates/globset"
    ))]
    InvalidWildcard {
        /// The error that occurred.
        error: String,
    },

    /// An attribute reference index is out of bounds for the registry's attribute catalog.
    #[error(
        "Invalid registry: {registry_name}. Unable to find attribute by index: {attribute_ref}"
    )]
    InvalidRegistryAttributeRef {
        /// The registry with the issue.
        registry_name: String,
        /// The attribute index that does not exist in the registry.
        attribute_ref: u32,
    },

    /// Dependencies declare incompatible versions of the same registry.
    #[error("Incompatible dependency versions for '{name}': {version1} and {version2}")]
    IncompatibleDependencyVersions {
        /// The name of the dependency.
        name: String,
        /// The first version found.
        version1: String,
        /// The second version found.
        version2: String,
    },

    /// Attribute was removed or missing in an upgraded dependency version.
    #[error("Attribute '{attribute_name}' defined in '{original_url}' was not found in upgraded dependency schema '{upgraded_url}'.")]
    #[diagnostic(help(
        "Ensure that upgraded schema `{upgraded_url}` provides attribute `{attribute_name}`, or update the reference."
    ))]
    AttributeNotFoundInUpgradedSchema {
        /// Name of the attribute.
        attribute_name: String,
        /// Original dependency schema URL.
        original_url: String,
        /// Chosen upgraded dependency schema URL.
        upgraded_url: String,
    },

    /// Group was removed or missing in an upgraded dependency version.
    #[error("Group '{group_id}' defined in '{original_url}' was not found in upgraded dependency schema '{upgraded_url}'.")]
    #[diagnostic(help(
        "Ensure that upgraded schema `{upgraded_url}` provides group `{group_id}`, or update the reference."
    ))]
    GroupNotFoundInUpgradedSchema {
        /// ID of the group/signal.
        group_id: String,
        /// Original dependency schema URL.
        original_url: String,
        /// Chosen upgraded dependency schema URL.
        upgraded_url: String,
    },

    /// Entity was removed or missing in an upgraded dependency version.
    #[error("Entity '{entity_name}' defined in '{original_url}' was not found in upgraded dependency schema '{upgraded_url}'.")]
    #[diagnostic(help(
        "Ensure that upgraded schema `{upgraded_url}` provides entity `{entity_name}`, or update the reference."
    ))]
    EntityNotFoundInUpgradedSchema {
        /// Name of the entity.
        entity_name: String,
        /// Original dependency schema URL.
        original_url: String,
        /// Chosen upgraded dependency schema URL.
        upgraded_url: String,
    },

    /// A chosen dependency schema should have been pre-resolved into cache, but was not.
    #[error("Chosen schema `{schema_url}` was not loaded into cache prior to resolution. This is a bug in weaver.")]
    #[diagnostic(
        code(weaver_resolver::chosen_schema_not_in_cache),
        help("This is an internal Weaver error. Please report it.")
    )]
    ChosenSchemaNotInCache {
        /// The schema URL that was expected in cache.
        schema_url: SchemaUrl,
    },

    /// Emitted when a dependency version resolved across the workspace differs from what was declared in main.
    #[error("Selected version {selected_version} for dependency '{dependency}' differs from requested in main ({requested_version}).")]
    #[diagnostic(severity(Warning))]
    DependencyVersionUpgradedWarning {
        /// Dependency name.
        dependency: String,
        /// Declared version.
        requested_version: String,
        /// Selected version across the graph.
        selected_version: String,
    },

    /// A reference matches definitions in multiple dependencies with different schema URLs.
    #[error("Ambiguous reference '{ref}' found in multiple dependencies with different SchemaURLs: {schema_url1} and {schema_url2}")]
    AmbiguousReference {
        /// The reference that is ambiguous.
        r#ref: String,
        /// The first SchemaURL found.
        schema_url1: String,
        /// The second SchemaURL found.
        schema_url2: String,
    },
    /// Deprecated `include_unreferenced` usage warning.
    #[error("The flag `include_unreferenced` is deprecated. Please prefer manually adding the required imports to your schema files in the future.")]
    #[diagnostic(severity(Warning))]
    DeprecatedIncludeUnreferencedWarning {},

    /// Mismatch between expected and actual schema URL.
    #[error("Mismatch between expected schema URL `{expected}` and actual schema URL `{actual}` resolved from definition.\nThis is likely because you are using schema_url overrides and trying to replace one version with another.")]
    #[diagnostic(severity(Error))]
    MismatchSchemaUrl {
        /// The expected schema URL.
        expected: String,
        /// The actual schema URL resolved.
        actual: String,
    },

    /// A container for multiple errors.
    #[error("{}", format_errors(.0))]
    CompoundError(#[related] Vec<Error>),

    /// Attempted to convert an OpenTelemetry V2 schema into a V1 schema.
    #[error("Converting a V2 schema into a V1 schema is unsupported")]
    ConvertingV2ToV1Unsupported,

    /// A user-defined schema loading visitor hook aborted the resolution pass.
    #[error("Schema loading aborted by visitor hook")]
    LoadingAbortedByVisitor,

    /// Registry mixes definition/1 and definition/2 files and falls back to V1 resolution.
    #[error("Registry `{schema_url}` mixes definition/1 and definition/2 files; using the V1 resolution path")]
    #[diagnostic(severity(Warning))]
    #[diagnostic(help(
        "Migrate all files in registry `{schema_url}` to definition/2 to use the native V2 resolution path."
    ))]
    MixedRegistryUsesV1Path {
        /// The schema URL of the registry.
        schema_url: String,
    },

    /// A dependency uses definition/1 or mixed format and falls back to V1 resolution.
    #[error("Dependency `{schema_url}` uses {format} format; using the V1 resolution path")]
    #[diagnostic(severity(Warning))]
    V1DependencyUsesV1Path {
        /// The schema URL of the dependency.
        schema_url: String,
        /// The source format of the dependency.
        format: SourceFormat,
    },
}

impl WeaverError<Error> for Error {
    fn compound(errors: Vec<Error>) -> Error {
        Self::CompoundError(
            errors
                .into_iter()
                .flat_map(|e| match e {
                    Self::CompoundError(errors) => errors,
                    e => vec![e],
                })
                .collect(),
        )
    }
}

impl From<Error> for DiagnosticMessages {
    fn from(error: Error) -> Self {
        DiagnosticMessages::new(match error {
            Error::CompoundError(errors) => errors
                .into_iter()
                .flat_map(|e| {
                    let diag_msgs: DiagnosticMessages = e.into();
                    diag_msgs.into_inner()
                })
                .collect(),
            _ => vec![DiagnosticMessage::new(error)],
        })
    }
}

impl Error {
    /// Logs this error, expanding `CompoundError` entries individually.
    pub fn log(&self) {
        match self {
            Error::CompoundError(errors) => {
                for error in errors {
                    error.log();
                }
            }
            _ => log_error(self),
        }
    }
}

impl From<weaver_semconv::Error> for Error {
    fn from(e: weaver_semconv::Error) -> Self {
        match e {
            weaver_semconv::Error::FailToResolveSchemaUrl {} => Error::FailToResolveSchemaUrl {},
            other => Error::FailToResolveDefinition(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Error;
    use weaver_common::{diagnostic::DiagnosticMessages, error::handle_errors};

    #[test]
    fn test_weaver_error_api_allowed() {
        let errors = vec![Error::FailToResolveMetric {
            r#ref: "test".to_owned(),
        }];
        let result = handle_errors(errors);
        assert!(result.is_err());
    }
    #[test]
    fn test_diagnostic_message_api_conversion() {
        let _: DiagnosticMessages = Error::FailToResolveMetric {
            r#ref: "test".to_owned(),
        }
        .into();
    }
}
