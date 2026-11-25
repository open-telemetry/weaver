// SPDX-License-Identifier: Apache-2.0

//! Shared `imports:` category definitions and globset compilation helpers.

use globset::{Glob, GlobSet, GlobSetBuilder};
use weaver_semconv::v2::{GroupWildcard, Imports};

use crate::error::Error;

/// Signal or attribute-group category field within an `imports:` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ImportField {
    Metrics,
    Events,
    Spans,
    Entities,
    AttributeGroups,
}

impl ImportField {
    /// Returns all five importable definition categories in deterministic processing order.
    #[must_use]
    pub(crate) fn all() -> [Self; 5] {
        [
            Self::Metrics,
            Self::Events,
            Self::Spans,
            Self::Entities,
            Self::AttributeGroups,
        ]
    }

    /// Returns the YAML field name used in diagnostics.
    #[must_use]
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Metrics => "metrics",
            Self::Events => "events",
            Self::Spans => "spans",
            Self::Entities => "entities",
            Self::AttributeGroups => "attribute_groups",
        }
    }

    /// Returns the wildcard patterns configured for this category in a V2 `Imports` block.
    #[must_use]
    pub(crate) fn patterns<'a>(&self, imports: &'a Imports) -> &'a [GroupWildcard] {
        let field = match self {
            Self::Metrics => &imports.metrics,
            Self::Events => &imports.events,
            Self::Spans => &imports.spans,
            Self::Entities => &imports.entities,
            Self::AttributeGroups => &imports.attribute_groups,
        };
        field.as_deref().unwrap_or_default()
    }
}

/// Compiles an iterator of glob patterns into a single `GlobSet` matcher.
pub(crate) fn build_globset<'a>(
    globs: impl IntoIterator<Item = &'a Glob>,
) -> Result<GlobSet, Error> {
    let mut builder = GlobSetBuilder::new();
    for glob in globs {
        _ = builder.add(glob.clone());
    }
    builder.build().map_err(|e| Error::InvalidWildcard {
        error: e.to_string(),
    })
}
