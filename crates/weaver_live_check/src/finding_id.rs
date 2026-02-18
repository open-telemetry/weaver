// SPDX-License-Identifier: Apache-2.0

//! Finding ID enum for live check findings.
//!
//! This enum mirrors `weaver.finding.id` in `model/live_check.yaml`.
//! It will be replaced by code generation in a future phase.

use serde::{Deserialize, Serialize};

/// Unique identifier for the type of finding detected by the policy engine.
///
/// The finding ID is a stable, machine-readable identifier that categorizes
/// the issue. It can be used to filter, aggregate, or suppress specific
/// finding types.
///
/// Custom Rego policies may emit additional finding IDs not listed here,
/// represented by the `Custom` variant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(strum::Display, strum::EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum FindingId {
    // Built-in advisor findings
    /// An attribute is not defined in the semantic convention registry
    MissingAttribute,
    /// An attribute matched a template definition in the registry
    TemplateAttribute,
    /// A metric is not defined in the semantic convention registry
    MissingMetric,
    /// An event is not defined in the semantic convention registry
    MissingEvent,
    /// A deprecated attribute or signal is in use
    Deprecated,
    /// An attribute value type does not match the type defined in the registry
    TypeMismatch,
    /// An attribute or signal has not reached stable status
    NotStable,
    /// A metric unit does not match the unit defined in the registry
    UnitMismatch,
    /// A metric instrument type does not match or is not supported by the registry
    UnexpectedInstrument,
    /// An enum attribute value is not in the set of allowed members
    UndefinedEnumVariant,
    // Requirement-level attribute findings
    /// A required attribute is absent from the sample
    RequiredAttributeNotPresent,
    /// A recommended attribute is absent from the sample
    RecommendedAttributeNotPresent,
    /// An opt-in attribute is absent from the sample
    OptInAttributeNotPresent,
    /// A conditionally required attribute is absent from the sample
    ConditionallyRequiredAttributeNotPresent,
    // Default Rego policy findings (otel.rego)
    /// An attribute name has no dot-separated namespace
    MissingNamespace,
    /// An attribute or metric name does not match the required naming format
    InvalidFormat,
    /// An attribute name uses a namespace that collides with an existing attribute
    IllegalNamespace,
    /// An attribute name shares a namespace prefix with existing registry attributes
    ExtendsNamespace,
    /// A finding ID from a custom Rego policy or other external source
    #[serde(untagged)]
    #[strum(default)]
    Custom(String),
}

impl From<FindingId> for String {
    fn from(id: FindingId) -> Self {
        id.to_string()
    }
}
