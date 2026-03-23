//! Provenance tracking for v2.

use std::fmt::Display;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The provenance of a semantic convention attribute or signal.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Hash, Eq, Default)]
pub struct Provenance {
    /// The dependency that defined this attribute or signal.
    ///
    /// If empty, then the signal was defined locally for this registry.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<DependencyRef>,

    /// The path to the file that specified this attribute or signal.
    ///
    /// This is only available locally and is not included when publishing resolved schemas.
    ///
    /// We use this for good error messages within Weaver.
    #[serde(skip)]
    pub path: String,
}

impl Provenance {
    /// Returns true if this provenance is empty (i.e. the attribute or signal was defined locally).
    ///
    /// Note: Path is *not* serialized out, so this will be true for local attributes
    /// even if a path is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.source.is_none()
    }
}

/// Reference to a dependency in the dependency list of this catalog.
#[derive(
    Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, JsonSchema, Hash,
)]
pub struct DependencyRef(pub u32);

impl Display for DependencyRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DependencyRef({})", self.0)
    }
}
