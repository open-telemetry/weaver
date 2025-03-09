// Intermediary format for telemetry samples

/// Represents a sample telemetry attribute parsed from any source
#[derive(Debug, Clone, PartialEq)]
pub struct SampleAttribute {
    /// The name of the attribute
    pub name: String,
}
