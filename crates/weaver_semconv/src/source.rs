// SPDX-License-Identifier: Apache-2.0

//! The source of a semantic convention specification file.

use std::sync::Arc;

/// A source of a semantic convention specification file.
#[derive(Debug, Clone)]
pub struct Source {
    /// The registry id containing the specification file.
    pub registry_id: Arc<str>,
    /// The path to the specification file.
    pub path: String,
}