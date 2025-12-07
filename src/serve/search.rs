// SPDX-License-Identifier: Apache-2.0

//! Search functionality for the semantic convention registry.

use weaver_forge::v2::registry::ForgeResolvedRegistry;
use weaver_semconv::stability::Stability;

use super::types::{
    AttributeSearchResult, EntitySearchResult, EventSearchResult, MetricSearchResult, SearchResult,
    SearchType, SpanSearchResult,
};

/// Search context for performing fuzzy searches across the registry.
pub struct SearchContext {
    /// All searchable items indexed for fast lookup.
    items: Vec<SearchableItem>,
}

/// A searchable item from the registry.
struct SearchableItem {
    /// The type of item.
    item_type: SearchType,
    /// Primary identifier (key/name/type).
    id: String,
    /// Brief description.
    brief: String,
    /// Note/extended description.
    note: String,
    /// Stability level.
    stability: Stability,
    /// Additional type info (for attributes).
    type_info: Option<String>,
}

impl SearchContext {
    /// Build a search context from a resolved registry.
    pub fn from_registry(registry: &ForgeResolvedRegistry) -> Self {
        let mut items = Vec::new();

        // Index all attributes
        for attr in &registry.attributes {
            items.push(SearchableItem {
                item_type: SearchType::Attribute,
                id: attr.key.clone(),
                brief: attr.common.brief.clone(),
                note: attr.common.note.clone(),
                stability: attr.common.stability.clone(),
                type_info: Some(format!("{}", attr.r#type)),
            });
        }

        // Index all metrics
        for metric in &registry.signals.metrics {
            items.push(SearchableItem {
                item_type: SearchType::Metric,
                id: metric.name.to_string(),
                brief: metric.common.brief.clone(),
                note: metric.common.note.clone(),
                stability: metric.common.stability.clone(),
                type_info: None,
            });
        }

        // Index all spans
        for span in &registry.signals.spans {
            items.push(SearchableItem {
                item_type: SearchType::Span,
                id: span.r#type.to_string(),
                brief: span.common.brief.clone(),
                note: span.common.note.clone(),
                stability: span.common.stability.clone(),
                type_info: None,
            });
        }

        // Index all events
        for event in &registry.signals.events {
            items.push(SearchableItem {
                item_type: SearchType::Event,
                id: event.name.to_string(),
                brief: event.common.brief.clone(),
                note: event.common.note.clone(),
                stability: event.common.stability.clone(),
                type_info: None,
            });
        }

        // Index all entities
        for entity in &registry.signals.entities {
            items.push(SearchableItem {
                item_type: SearchType::Entity,
                id: entity.r#type.to_string(),
                brief: entity.common.brief.clone(),
                note: entity.common.note.clone(),
                stability: entity.common.stability.clone(),
                type_info: None,
            });
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
            .filter(|item| search_type == SearchType::All || item.item_type == search_type)
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
    fn to_search_result(&self, score: u32) -> SearchResult {
        match self.item_type {
            SearchType::Attribute => SearchResult::Attribute(AttributeSearchResult {
                key: self.id.clone(),
                brief: self.brief.clone(),
                attr_type: self.type_info.clone().unwrap_or_default(),
                stability: Some(self.stability.clone()),
                score,
            }),
            SearchType::Metric => SearchResult::Metric(MetricSearchResult {
                name: self.id.clone(),
                brief: self.brief.clone(),
                stability: Some(self.stability.clone()),
                score,
            }),
            SearchType::Span => SearchResult::Span(SpanSearchResult {
                span_type: self.id.clone(),
                brief: self.brief.clone(),
                stability: Some(self.stability.clone()),
                score,
            }),
            SearchType::Event => SearchResult::Event(EventSearchResult {
                name: self.id.clone(),
                brief: self.brief.clone(),
                stability: Some(self.stability.clone()),
                score,
            }),
            SearchType::Entity => SearchResult::Entity(EntitySearchResult {
                entity_type: self.id.clone(),
                brief: self.brief.clone(),
                stability: Some(self.stability.clone()),
                score,
            }),
            SearchType::All => {
                // This shouldn't happen since we filter by type
                SearchResult::Attribute(AttributeSearchResult {
                    key: self.id.clone(),
                    brief: self.brief.clone(),
                    attr_type: String::new(),
                    stability: Some(self.stability.clone()),
                    score,
                })
            }
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
fn score_match(query: &str, item: &SearchableItem) -> u32 {
    let query_lower = query.to_lowercase();
    let id_lower = item.id.to_lowercase();
    let brief_lower = item.brief.to_lowercase();
    let note_lower = item.note.to_lowercase();

    // Exact match
    if id_lower == query_lower {
        return 100;
    }

    // Name starts with query
    if id_lower.starts_with(&query_lower) {
        return 80;
    }

    // Name contains query
    if id_lower.contains(&query_lower) {
        return 70;
    }

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
            return 60;
        }
    }

    // Brief contains query
    if brief_lower.contains(&query_lower) {
        return 40;
    }

    // Note contains query
    if note_lower.contains(&query_lower) {
        return 20;
    }

    // Also check if individual query tokens appear in brief/note
    if !query_tokens.is_empty() {
        let all_in_brief = query_tokens.iter().all(|qt| brief_lower.contains(qt));
        if all_in_brief {
            return 35;
        }

        let all_in_note = query_tokens.iter().all(|qt| note_lower.contains(qt));
        if all_in_note {
            return 15;
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match_scores_highest() {
        let item = SearchableItem {
            item_type: SearchType::Attribute,
            id: "http.request.method".to_owned(),
            brief: "HTTP request method".to_owned(),
            note: String::new(),
            stability: Stability::Stable,
            type_info: None,
        };

        assert_eq!(score_match("http.request.method", &item), 100);
    }

    #[test]
    fn test_starts_with_scores_high() {
        let item = SearchableItem {
            item_type: SearchType::Attribute,
            id: "http.request.method".to_owned(),
            brief: "HTTP request method".to_owned(),
            note: String::new(),
            stability: Stability::Stable,
            type_info: None,
        };

        assert_eq!(score_match("http.request", &item), 80);
    }

    #[test]
    fn test_contains_scores_medium() {
        let item = SearchableItem {
            item_type: SearchType::Attribute,
            id: "http.request.method".to_owned(),
            brief: "HTTP request method".to_owned(),
            note: String::new(),
            stability: Stability::Stable,
            type_info: None,
        };

        assert_eq!(score_match("request.method", &item), 70);
    }

    #[test]
    fn test_brief_match_scores_lower() {
        let item = SearchableItem {
            item_type: SearchType::Attribute,
            id: "http.request.method".to_owned(),
            brief: "The HTTP verb used in the request".to_owned(),
            note: String::new(),
            stability: Stability::Stable,
            type_info: None,
        };

        assert_eq!(score_match("verb", &item), 40);
    }

    #[test]
    fn test_no_match_scores_zero() {
        let item = SearchableItem {
            item_type: SearchType::Attribute,
            id: "http.request.method".to_owned(),
            brief: "HTTP request method".to_owned(),
            note: String::new(),
            stability: Stability::Stable,
            type_info: None,
        };

        assert_eq!(score_match("database", &item), 0);
    }
}
