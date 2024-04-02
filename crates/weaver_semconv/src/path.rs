// SPDX-License-Identifier: Apache-2.0

//! Semantic convention registry path.

use serde::{Deserialize, Serialize};

/// A semantic convention registry path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegistryPath {
    /// A local path to the semantic convention registry.
    Local {
        /// The local path to the semantic convention directory.
        path: String,
    },
    /// A git URL to the semantic convention registry.
    GitUrl {
        /// The git URL of the semantic convention git repo.
        git_url: String,
        /// An optional path to the semantic convention directory containing
        /// the semantic convention files.
        path: Option<String>,
    },
}
