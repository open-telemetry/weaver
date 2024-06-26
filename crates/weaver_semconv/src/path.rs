// SPDX-License-Identifier: Apache-2.0

//! Semantic convention registry path.

use std::fmt::{Display, Formatter};

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

impl Display for RegistryPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let path = match self {
            RegistryPath::Local { path_pattern } => format!("LocalRegistry:{}", path_pattern),
            RegistryPath::GitUrl { git_url, path } => match path {
                Some(path) => format!("GitRegistry:{}/{:?}", git_url, path),
                None => format!("GitRegistry:{}", git_url),
            },
        };
        f.write_str(&path)
    }
}
