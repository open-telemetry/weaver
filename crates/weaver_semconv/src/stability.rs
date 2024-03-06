// SPDX-License-Identifier: Apache-2.0

//! Stability specification.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// The level of stability for a definition.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
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
