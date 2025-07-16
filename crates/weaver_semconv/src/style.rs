// SPDX-License-Identifier: Apache-2.0

//! Stability specification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// The level of stability for a definition. Defined in [OTEP-232](https://github.com/open-telemetry/oteps/blob/main/text/0232-maturity-of-otel.md)
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    /// A note message.
    Note,
    /// A tip message.
    Tip,
    /// An Important message.
    Important,
    /// A Warning message.
    Warning,
    /// A Caution message.
    Caution,
}

/// Implements a human readable display for the stability.
impl Display for Style {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Style::Note => write!(f, "NOTE"),
            Style::Tip => write!(f, "TIP"),
            Style::Important => write!(f, "IMPORTANT"),
            Style::Warning => write!(f, "WARNING"),
            Style::Caution => write!(f, "CAUTION"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_deserialize_stability() {
        let note: Style = serde_json::from_str("\"note\"").unwrap();
        assert_eq!(note, Style::Note);

        let tip: Style = serde_json::from_str("\"tip\"").unwrap();
        assert_eq!(tip, Style::Tip);

        let important: Style = serde_json::from_str("\"important\"").unwrap();
        assert_eq!(important, Style::Important);

        let warning: Style = serde_json::from_str("\"warning\"").unwrap();
        assert_eq!(warning, Style::Warning);

        let caution: Style = serde_json::from_str("\"caution\"").unwrap();
        assert_eq!(caution, Style::Caution);
    }

    #[test]
    fn test_display() {
        assert_eq!(Style::Note.to_string(), "NOTE");
        assert_eq!(Style::Tip.to_string(), "TIP");
        assert_eq!(Style::Important.to_string(), "IMPORTANT");
        assert_eq!(Style::Warning.to_string(), "WARNING");
        assert_eq!(Style::Caution.to_string(), "CAUTION");
    }
}
