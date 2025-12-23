// SPDX-License-Identifier: Apache-2.0

//! Stability specification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// The level of stability for a definition. Defined in [OTEP-232](https://github.com/open-telemetry/oteps/blob/main/text/0232-maturity-of-otel.md)
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    /// A deprecated definition.
    #[deprecated(note = "This stability level is deprecated.")]
    Deprecated,
    /// A stable definition.
    Stable,
    /// A definition in development. Formally known as experimental.
    #[serde(alias = "experimental")]
    Development,
    /// An alpha definition.
    Alpha,
    /// A beta definition.
    Beta,
    /// A release candidate definition.
    ReleaseCandidate,
}

/// Implements a human readable display for the stability.
impl Display for Stability {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Stability::Stable => write!(f, "stable"),
            Stability::Development => write!(f, "development"),
            Stability::Alpha => write!(f, "alpha"),
            Stability::Beta => write!(f, "beta"),
            Stability::ReleaseCandidate => write!(f, "release_candidate"),
            Stability::Deprecated => write!(f, "deprecated"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_deserialize_stability() {
        let deprecated: Stability = serde_json::from_str("\"deprecated\"").unwrap();
        assert_eq!(deprecated, Stability::Deprecated);

        let stable: Stability = serde_json::from_str("\"stable\"").unwrap();
        assert_eq!(stable, Stability::Stable);

        let development: Stability = serde_json::from_str("\"development\"").unwrap();
        assert_eq!(development, Stability::Development);

        let experimental: Stability = serde_json::from_str("\"experimental\"").unwrap();
        assert_eq!(experimental, Stability::Development);

        let alpha: Stability = serde_json::from_str("\"alpha\"").unwrap();
        assert_eq!(alpha, Stability::Alpha);

        let beta: Stability = serde_json::from_str("\"beta\"").unwrap();
        assert_eq!(beta, Stability::Beta);

        let release_candidate: Stability = serde_json::from_str("\"release_candidate\"").unwrap();
        assert_eq!(release_candidate, Stability::ReleaseCandidate);
    }

    #[test]
    fn test_display() {
        assert_eq!(Stability::Stable.to_string(), "stable");
        assert_eq!(Stability::Development.to_string(), "development");
        assert_eq!(Stability::Alpha.to_string(), "alpha");
        assert_eq!(Stability::Beta.to_string(), "beta");
        assert_eq!(Stability::ReleaseCandidate.to_string(), "release_candidate");
    }
}
