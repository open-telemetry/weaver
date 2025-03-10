// Intermediary format for telemetry samples

use serde::Serialize;

/// Represents a sample telemetry attribute parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SampleAttribute {
    /// The name of the attribute
    pub name: String,
}
