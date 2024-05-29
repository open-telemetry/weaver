// SPDX-License-Identifier: Apache-2.0

//! Stability specification.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use schemars::JsonSchema;

/// The level of stability for a definition.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    /// A deprecated definition.
    Deprecated,
    /// An experimental definition.
    Experimental,
    /// A stable definition.
    Stable,
}

/// Implements a human readable display for the stability.
impl Display for Stability {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Stability::Deprecated => write!(f, "deprecated"),
            Stability::Experimental => write!(f, "experimental"),
            Stability::Stable => write!(f, "stable"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        assert_eq!(Stability::Deprecated.to_string(), "deprecated");
        assert_eq!(Stability::Experimental.to_string(), "experimental");
        assert_eq!(Stability::Stable.to_string(), "stable");
    }
}
