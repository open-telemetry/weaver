// SPDX-License-Identifier: Apache-2.0

//! Stability specification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::style::Style;

/// A header specification
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HeaderSpec {

    /// The title of the header.
    pub title: Option<String>,
    /// The content in the header.
    pub content: String,
    /// The style of the header.
    pub style: Style,
}

impl HeaderSpec {
    /// returns the title of the header
    #[must_use]
    fn title(&self) -> &Option<String> {
        &self.title
    }
    /// returns the content of the header
    #[must_use]
    fn content(&self) -> &String {
        &self.content
    }
    /// returns the style of the header
    #[must_use]
    fn style(&self) -> &Style {
        &self.style
    }
}
