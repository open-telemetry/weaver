// SPDX-License-Identifier: Apache-2.0

//! Requirement level for signals (metrics, spans, events, entities).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// The requirement level of a signal (metric, span, event, entity). It is
/// guidance for instrumentation authors about what should be emitted by default
/// versus what should be opt-in. Unlike attribute requirement levels, signals
/// only support these two levels.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, Default)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum SignalRequirementLevel {
    /// Instrumentations should implement and emit the signal by default.
    /// This is the default for signals that do not specify a requirement level.
    #[default]
    Recommended,
    /// Disabled by default; instrumentations MAY implement the signal.
    OptIn,
}

/// Implements a human readable display for the signal requirement level.
impl Display for SignalRequirementLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalRequirementLevel::Recommended => write!(f, "recommended"),
            SignalRequirementLevel::OptIn => write!(f, "opt_in"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_signal_requirement_level() {
        let recommended: SignalRequirementLevel = serde_json::from_str("\"recommended\"").unwrap();
        assert_eq!(recommended, SignalRequirementLevel::Recommended);

        let opt_in: SignalRequirementLevel = serde_json::from_str("\"opt_in\"").unwrap();
        assert_eq!(opt_in, SignalRequirementLevel::OptIn);
    }

    #[test]
    fn test_display() {
        assert_eq!(
            SignalRequirementLevel::Recommended.to_string(),
            "recommended"
        );
        assert_eq!(SignalRequirementLevel::OptIn.to_string(), "opt_in");
    }

    #[test]
    fn test_default() {
        assert_eq!(
            SignalRequirementLevel::default(),
            SignalRequirementLevel::Recommended
        );
    }
}
