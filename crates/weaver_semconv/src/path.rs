// SPDX-License-Identifier: Apache-2.0

//! Semantic convention registry path.

use serde::{Deserialize, Serialize};

/// A semantic convention registry path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum RegistryPath {
    /// A local path pattern to the semantic convention registry.
    Local {
        /// A local path pattern to the semantic convention files.
        path_pattern: String,
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
