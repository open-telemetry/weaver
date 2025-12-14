// SPDX-License-Identifier: Apache-2.0

//! Search functionality for the semantic convention registry.

use std::sync::Arc;

use weaver_forge::v2::attribute::Attribute;
use weaver_forge::v2::entity::Entity;
use weaver_forge::v2::event::Event;
use weaver_forge::v2::metric::Metric;
use weaver_forge::v2::registry::ForgeResolvedRegistry;
use weaver_forge::v2::span::Span;

use super::types::{ScoredResult, SearchResult, SearchType};

/// Search context for performing fuzzy searches across the registry.
pub struct SearchContext {
    /// All searchable items indexed for fast lookup.
    items: Vec<SearchableItem>,
}

/// A searchable item from the registry containing the full object.
enum SearchableItem {
    /// An attribute with all its properties.
    Attribute(Arc<Attribute>),
    /// A metric with all its properties.
    Metric(Arc<Metric>),
    /// A span with all its properties.
    Span(Arc<Span>),
    /// An event with all its properties.
    Event(Arc<Event>),
    /// An entity with all its properties.
    Entity(Arc<Entity>),
}

impl SearchContext {
    /// Build a search context from a resolved registry.
    pub fn from_registry(registry: &ForgeResolvedRegistry) -> Self {
        let mut items = Vec::new();

        // Index all attributes
        for attr in &registry.attributes {
            items.push(SearchableItem::Attribute(Arc::new(attr.clone())));
        }

        // Index all metrics
        for metric in &registry.signals.metrics {
            items.push(SearchableItem::Metric(Arc::new(metric.clone())));
        }

        // Index all spans
        for span in &registry.signals.spans {
            items.push(SearchableItem::Span(Arc::new(span.clone())));
        }

        // Index all events
        for event in &registry.signals.events {
            items.push(SearchableItem::Event(Arc::new(event.clone())));
        }

        // Index all entities
        for entity in &registry.signals.entities {
            items.push(SearchableItem::Entity(Arc::new(entity.clone())));
        }

        Self { items }
    }

    /// Search for items matching the query.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query string.
    /// * `search_type` - Filter by item type.
    /// * `limit` - Maximum number of results.
    pub fn search(&self, query: &str, search_type: SearchType, limit: usize) -> Vec<SearchResult> {
        let limit = limit.min(200); // Cap at 200

        let mut scored_items: Vec<(u32, &SearchableItem)> = self
            .items
            .iter()
            .filter(|item| search_type == SearchType::All || item.search_type() == search_type)
            .filter_map(|item| {
                let score = score_match(query, item);
                if score > 0 {
                    Some((score, item))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score descending
        scored_items.sort_by(|a, b| b.0.cmp(&a.0));

        // Take top N and convert to results
        scored_items
            .into_iter()
            .take(limit)
            .map(|(score, item)| item.to_search_result(score))
            .collect()
    }
}

impl SearchableItem {
    /// Get the search type of this item.
    fn search_type(&self) -> SearchType {
        match self {
            SearchableItem::Attribute(_) => SearchType::Attribute,
            SearchableItem::Metric(_) => SearchType::Metric,
            SearchableItem::Span(_) => SearchType::Span,
            SearchableItem::Event(_) => SearchType::Event,
            SearchableItem::Entity(_) => SearchType::Entity,
        }
    }

    /// Get the primary identifier for scoring (key/name/type).
    fn id(&self) -> &str {
        match self {
            SearchableItem::Attribute(attr) => &attr.key,
            SearchableItem::Metric(metric) => &metric.name,
            SearchableItem::Span(span) => &span.r#type,
            SearchableItem::Event(event) => &event.name,
            SearchableItem::Entity(entity) => &entity.r#type,
        }
    }

    /// Get the brief description for scoring.
    fn brief(&self) -> &str {
        match self {
            SearchableItem::Attribute(attr) => &attr.common.brief,
            SearchableItem::Metric(metric) => &metric.common.brief,
            SearchableItem::Span(span) => &span.common.brief,
            SearchableItem::Event(event) => &event.common.brief,
            SearchableItem::Entity(entity) => &entity.common.brief,
        }
    }

    /// Get the note for scoring.
    fn note(&self) -> &str {
        match self {
            SearchableItem::Attribute(attr) => &attr.common.note,
            SearchableItem::Metric(metric) => &metric.common.note,
            SearchableItem::Span(span) => &span.common.note,
            SearchableItem::Event(event) => &event.common.note,
            SearchableItem::Entity(entity) => &entity.common.note,
        }
    }

    /// Check if this item is deprecated.
    fn is_deprecated(&self) -> bool {
        match self {
            SearchableItem::Attribute(attr) => attr.common.deprecated.is_some(),
            SearchableItem::Metric(metric) => metric.common.deprecated.is_some(),
            SearchableItem::Span(span) => span.common.deprecated.is_some(),
            SearchableItem::Event(event) => event.common.deprecated.is_some(),
            SearchableItem::Entity(entity) => entity.common.deprecated.is_some(),
        }
    }

    /// Convert to a search result with the given score.
    fn to_search_result(&self, score: u32) -> SearchResult {
        match self {
            SearchableItem::Attribute(attr) => SearchResult::Attribute(ScoredResult {
                score,
                item: Arc::clone(attr),
            }),
            SearchableItem::Metric(metric) => SearchResult::Metric(ScoredResult {
                score,
                item: Arc::clone(metric),
            }),
            SearchableItem::Span(span) => SearchResult::Span(ScoredResult {
                score,
                item: Arc::clone(span),
            }),
            SearchableItem::Event(event) => SearchResult::Event(ScoredResult {
                score,
                item: Arc::clone(event),
            }),
            SearchableItem::Entity(entity) => SearchResult::Entity(ScoredResult {
                score,
                item: Arc::clone(entity),
            }),
        }
    }
}

/// Calculate a relevance score for a search match.
///
/// Scoring weights:
/// - Exact name/key match: 100 points
/// - Name/key starts with query: 80 points
/// - Name/key contains query: 70 points
/// - All query tokens found in name: 60 points
/// - Brief contains query: 40 points
/// - Note contains query: 20 points
/// - Deprecated items: score divided by 10 (heavily demoted)
fn score_match(query: &str, item: &SearchableItem) -> u32 {
    let query_lower = query.to_lowercase();
    let id_lower = item.id().to_lowercase();
    let brief_lower = item.brief().to_lowercase();
    let note_lower = item.note().to_lowercase();

    let mut score = 0;

    // Exact match
    if id_lower == query_lower {
        score = 100;
    }
    // Name starts with query
    else if id_lower.starts_with(&query_lower) {
        score = 80;
    }
    // Name contains query
    else if id_lower.contains(&query_lower) {
        score = 70;
    } else {
        // Token matching - all query tokens found in name
        let query_tokens: Vec<&str> = query_lower
            .split(|c: char| c == '.' || c == '_' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect();

        if !query_tokens.is_empty() {
            let id_tokens: Vec<&str> = id_lower.split(['.', '_']).collect();

            let all_tokens_match = query_tokens
                .iter()
                .all(|qt| id_tokens.iter().any(|it| it.contains(qt)));

            if all_tokens_match {
                score = 60;
            }
            // Brief contains query
            else if brief_lower.contains(&query_lower) {
                score = 40;
            }
            // Note contains query
            else if note_lower.contains(&query_lower) {
                score = 20;
            }
            // Also check if individual query tokens appear in brief/note
            else {
                let all_in_brief = query_tokens.iter().all(|qt| brief_lower.contains(qt));
                if all_in_brief {
                    score = 35;
                } else {
                    let all_in_note = query_tokens.iter().all(|qt| note_lower.contains(qt));
                    if all_in_note {
                        score = 15;
                    }
                }
            }
        } else {
            // Brief contains query
            if brief_lower.contains(&query_lower) {
                score = 40;
            }
            // Note contains query
            else if note_lower.contains(&query_lower) {
                score = 20;
            }
        }
    }

    // Heavily demote deprecated items - divide score by 10
    if item.is_deprecated() && score > 0 {
        score /= 10;
        // Ensure at least 1 if there was a match
        if score == 0 {
            score = 1;
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use weaver_semconv::attribute::AttributeType;
    use weaver_semconv::deprecated::Deprecated;
    use weaver_semconv::stability::Stability;
    use weaver_semconv::v2::CommonFields;

    fn make_test_attribute(key: &str, brief: &str, note: &str, deprecated: bool) -> SearchableItem {
        SearchableItem::Attribute(Arc::new(Attribute {
            key: key.to_owned(),
            r#type: AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: brief.to_owned(),
                note: note.to_owned(),
                stability: Stability::Stable,
                deprecated: if deprecated {
                    Some(Deprecated::Obsoleted {
                        note: "Deprecated".to_owned(),
                    })
                } else {
                    None
                },
                annotations: BTreeMap::new(),
            },
        }))
    }

    #[test]
    fn test_exact_match_scores_highest() {
        let item = make_test_attribute("http.request.method", "HTTP request method", "", false);

        assert_eq!(score_match("http.request.method", &item), 100);
    }

    #[test]
    fn test_starts_with_scores_high() {
        let item = make_test_attribute("http.request.method", "HTTP request method", "", false);

        assert_eq!(score_match("http.request", &item), 80);
    }

    #[test]
    fn test_contains_scores_medium() {
        let item = make_test_attribute("http.request.method", "HTTP request method", "", false);

        assert_eq!(score_match("request.method", &item), 70);
    }

    #[test]
    fn test_brief_match_scores_lower() {
        let item = make_test_attribute(
            "http.request.method",
            "The HTTP verb used in the request",
            "",
            false,
        );

        assert_eq!(score_match("verb", &item), 40);
    }

    #[test]
    fn test_no_match_scores_zero() {
        let item = make_test_attribute("http.request.method", "HTTP request method", "", false);

        assert_eq!(score_match("database", &item), 0);
    }

    #[test]
    fn test_deprecated_items_score_much_lower() {
        let item = make_test_attribute("http.request.method", "HTTP request method", "", true);

        // Exact match for deprecated item: 100 / 10 = 10
        assert_eq!(score_match("http.request.method", &item), 10);

        // Starts with for deprecated item: 80 / 10 = 8
        assert_eq!(score_match("http.request", &item), 8);
    }
}
