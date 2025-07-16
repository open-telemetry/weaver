// SPDX-License-Identifier: Apache-2.0

//! Stability specification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::style::Style;

/// A header specification
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FooterSpec {

    /// The title of the footer.
    pub title: Option<String>,
    /// The content in the footer.
    pub content: String,
    /// The style of the footer.
    pub style: Option<Style>,
}

impl FooterSpec {
    /// returns the title of the footer
    #[must_use]
    fn title(&self) -> &Option<String> {
        &self.title
    }
    /// returns the content of the footer
    #[must_use]
    fn content(&self) -> &String {
        &self.content
    }
    /// returns the style of the footer
    #[must_use]
    fn style(&self) -> &Option<Style> {
        &self.style
    }
}
