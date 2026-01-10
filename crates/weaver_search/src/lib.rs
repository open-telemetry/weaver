// SPDX-License-Identifier: Apache-2.0

//! Search functionality for the semantic convention registry.
//!
//! This crate provides a search engine for querying OpenTelemetry semantic
//! convention registries. It supports fuzzy matching, type filtering, and
//! stability filtering.

#![doc = include_str!("../README.md")]

mod types;

pub use types::{ScoredResult, SearchResult, SearchType};

use std::collections::HashMap;
use std::sync::Arc;

use weaver_forge::v2::{
    attribute::Attribute, entity::Entity, event::Event, metric::Metric,
    registry::ForgeResolvedRegistry, span::Span,
};
use weaver_semconv::attribute::AttributeType;
use weaver_semconv::stability::Stability;

//TODO: Consider using a fuzzy matching crate for improved search capabilities.
// e.g. Tantivy - https://github.com/open-telemetry/weaver/pull/1076#discussion_r2640681775

/// Search context for performing fuzzy searches and O(1) lookups across the registry.
pub struct SearchContext {
    /// All searchable items for fuzzy search.
    items: Vec<SearchableItem>,

    // O(1) lookup indices (following LiveChecker pattern)
    /// Attributes indexed by key.
    attr_index: HashMap<String, Arc<Attribute>>,
    /// Template attributes indexed by key.
    template_index: HashMap<String, Arc<Attribute>>,
    /// Templates sorted by key length (longest first) for prefix matching.
    templates_by_length: Vec<(String, Arc<Attribute>)>,
    /// Metrics indexed by name.
    metric_index: HashMap<String, Arc<Metric>>,
    /// Spans indexed by type.
    span_index: HashMap<String, Arc<Span>>,
    /// Events indexed by name.
    event_index: HashMap<String, Arc<Event>>,
    /// Entities indexed by type.
    entity_index: HashMap<String, Arc<Entity>>,
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
    #[must_use]
    pub fn from_registry(registry: &ForgeResolvedRegistry) -> Self {
        let mut items = Vec::new();
        let mut attr_index = HashMap::new();
        let mut template_index = HashMap::new();
        let mut templates_by_length = Vec::new();
        let mut metric_index = HashMap::new();
        let mut span_index = HashMap::new();
        let mut event_index = HashMap::new();
        let mut entity_index = HashMap::new();

        // Index all attributes
        for attr in &registry.registry.attributes {
            let arc_attr = Arc::new(attr.clone());
            items.push(SearchableItem::Attribute(Arc::clone(&arc_attr)));

            // Separate templates from regular attributes (following LiveChecker pattern)
            if matches!(attr.r#type, AttributeType::Template(_)) {
                let _ = template_index.insert(attr.key.clone(), Arc::clone(&arc_attr));
                templates_by_length.push((attr.key.clone(), arc_attr));
            } else {
                let _ = attr_index.insert(attr.key.clone(), arc_attr);
            }
        }

        // Sort templates by key length descending (longest first for prefix matching)
        templates_by_length.sort_by(|(a, _), (b, _)| b.len().cmp(&a.len()));

        // Index all metrics
        for metric in &registry.registry.metrics {
            let arc_metric = Arc::new(metric.clone());
            items.push(SearchableItem::Metric(Arc::clone(&arc_metric)));
            let _ = metric_index.insert(metric.name.to_string(), arc_metric);
        }

        // Index all spans
        for span in &registry.registry.spans {
            let arc_span = Arc::new(span.clone());
            items.push(SearchableItem::Span(Arc::clone(&arc_span)));
            let _ = span_index.insert(span.r#type.to_string(), arc_span);
        }

        // Index all events
        for event in &registry.registry.events {
            let arc_event = Arc::new(event.clone());
            items.push(SearchableItem::Event(Arc::clone(&arc_event)));
            let _ = event_index.insert(event.name.to_string(), arc_event);
        }

        // Index all entities
        for entity in &registry.registry.entities {
            let arc_entity = Arc::new(entity.clone());
            items.push(SearchableItem::Entity(Arc::clone(&arc_entity)));
            let _ = entity_index.insert(entity.r#type.to_string(), arc_entity);
        }

        Self {
            items,
            attr_index,
            template_index,
            templates_by_length,
            metric_index,
            span_index,
            event_index,
            entity_index,
        }
    }

    /// Search for items matching the query, or list all items if query is None.
    ///
    /// # Arguments
    ///
    /// * `query` - Optional search query string (None = browse mode).
    /// * `search_type` - Filter by item type.
    /// * `stability` - Optional stability filter.
    /// * `limit` - Maximum number of results.
    /// * `offset` - Pagination offset.
    ///
    /// # Returns
    ///
    /// Tuple of (results, total_count) for pagination.
    #[must_use]
    pub fn search(
        &self,
        query: Option<&str>,
        search_type: SearchType,
        stability: Option<Stability>,
        limit: usize,
        offset: usize,
    ) -> (Vec<SearchResult>, usize) {
        let limit = limit.min(200); // Cap at 200

        // Filter by type
        let mut items: Vec<&SearchableItem> = self
            .items
            .iter()
            .filter(|item| search_type == SearchType::All || item.search_type() == search_type)
            .collect();

        // Filter by stability if provided
        if let Some(stability_filter) = stability {
            items.retain(|item| item.stability() == &stability_filter);
        }

        // Branch based on whether we have a search query
        let (results, total) = if let Some(q) = query {
            if q.is_empty() {
                // Empty query - browse mode
                let total = items.len();
                let results = browse_mode(items, limit, offset);
                (results, total)
            } else {
                // Non-empty query - search mode with scoring
                search_mode_with_total(items, q, limit)
            }
        } else {
            // No query - browse mode
            let total = items.len();
            let results = browse_mode(items, limit, offset);
            (results, total)
        };

        (results, total)
    }

    // ==========================================================================
    // O(1) Lookup Methods (following LiveChecker pattern)
    // ==========================================================================

    /// Get an attribute by exact key match. O(1) lookup.
    #[must_use]
    pub fn get_attribute(&self, key: &str) -> Option<Arc<Attribute>> {
        self.attr_index.get(key).map(Arc::clone)
    }

    /// Get a template attribute by exact key match. O(1) lookup.
    #[must_use]
    pub fn get_template(&self, key: &str) -> Option<Arc<Attribute>> {
        self.template_index.get(key).map(Arc::clone)
    }

    /// Find a template attribute matching the given attribute name prefix.
    /// Uses longest-prefix matching (e.g., "test.template.foo" matches "test.template").
    /// This follows the LiveChecker pattern for template resolution.
    #[must_use]
    pub fn find_template(&self, attribute_name: &str) -> Option<Arc<Attribute>> {
        for (template_key, attr) in &self.templates_by_length {
            if attribute_name.starts_with(template_key) {
                return Some(Arc::clone(attr));
            }
        }
        None
    }

    /// Get a metric by exact name match. O(1) lookup.
    #[must_use]
    pub fn get_metric(&self, name: &str) -> Option<Arc<Metric>> {
        self.metric_index.get(name).map(Arc::clone)
    }

    /// Get a span by exact type match. O(1) lookup.
    #[must_use]
    pub fn get_span(&self, span_type: &str) -> Option<Arc<Span>> {
        self.span_index.get(span_type).map(Arc::clone)
    }

    /// Get an event by exact name match. O(1) lookup.
    #[must_use]
    pub fn get_event(&self, name: &str) -> Option<Arc<Event>> {
        self.event_index.get(name).map(Arc::clone)
    }

    /// Get an entity by exact type match. O(1) lookup.
    #[must_use]
    pub fn get_entity(&self, entity_type: &str) -> Option<Arc<Entity>> {
        self.entity_index.get(entity_type).map(Arc::clone)
    }
}

/// Search mode with total count: perform fuzzy matching with scoring and return (results, total).
fn search_mode_with_total(
    items: Vec<&SearchableItem>,
    query: &str,
    limit: usize,
) -> (Vec<SearchResult>, usize) {
    let mut scored_items: Vec<(u32, &SearchableItem)> = items
        .into_iter()
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

    // Calculate total before taking limit
    let total = scored_items.len();

    // Take top N and convert to results
    let results = scored_items
        .into_iter()
        .take(limit)
        .map(|(score, item)| item.to_search_result(score))
        .collect();

    (results, total)
}

/// Browse mode: return all items in natural order with pagination.
fn browse_mode(items: Vec<&SearchableItem>, limit: usize, offset: usize) -> Vec<SearchResult> {
    items
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|item| item.to_search_result(0)) // Score 0 in browse mode
        .collect()
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

    /// Get the stability level of this item.
    fn stability(&self) -> &Stability {
        match self {
            SearchableItem::Attribute(attr) => &attr.common.stability,
            SearchableItem::Metric(metric) => &metric.common.stability,
            SearchableItem::Span(span) => &span.common.stability,
            SearchableItem::Event(event) => &event.common.stability,
            SearchableItem::Entity(entity) => &entity.common.stability,
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
    use weaver_forge::v2::registry::{ForgeResolvedRegistry, Refinements, Signals};
    use weaver_semconv::attribute::AttributeType;
    use weaver_semconv::deprecated::Deprecated;
    use weaver_semconv::group::{InstrumentSpec, SpanKindSpec};
    use weaver_semconv::stability::Stability;
    use weaver_semconv::v2::span::SpanName;
    use weaver_semconv::v2::CommonFields;

    fn make_test_attribute(key: &str, brief: &str, note: &str, deprecated: bool) -> SearchableItem {
        SearchableItem::Attribute(Arc::new(make_attribute(key, brief, note, deprecated)))
    }

    fn make_attribute(key: &str, brief: &str, note: &str, deprecated: bool) -> Attribute {
        Attribute {
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
        }
    }

    fn make_template_attribute(key: &str, brief: &str) -> Attribute {
        Attribute {
            key: key.to_owned(),
            r#type: AttributeType::Template(weaver_semconv::attribute::TemplateTypeSpec::String),
            examples: None,
            common: CommonFields {
                brief: brief.to_owned(),
                note: "".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: BTreeMap::new(),
            },
        }
    }

    fn make_development_attribute(key: &str, brief: &str) -> Attribute {
        Attribute {
            key: key.to_owned(),
            r#type: AttributeType::PrimitiveOrArray(
                weaver_semconv::attribute::PrimitiveOrArrayTypeSpec::String,
            ),
            examples: None,
            common: CommonFields {
                brief: brief.to_owned(),
                note: "".to_owned(),
                stability: Stability::Development,
                deprecated: None,
                annotations: BTreeMap::new(),
            },
        }
    }

    fn make_test_registry() -> ForgeResolvedRegistry {
        ForgeResolvedRegistry {
            registry_url: "test".to_owned(),
            attributes: vec![
                make_attribute("http.request.method", "HTTP request method", "", false),
                make_attribute(
                    "http.response.status_code",
                    "HTTP response status code",
                    "",
                    false,
                ),
                make_attribute(
                    "db.system",
                    "Database system",
                    "The database management system",
                    false,
                ),
                // Template attribute for testing get_template/find_template
                make_template_attribute("test.template", "A template attribute"),
                // Development stability attribute for testing stability filtering
                make_development_attribute("experimental.feature", "An experimental feature"),
            ],
            attribute_groups: vec![],
            signals: Signals {
                metrics: vec![Metric {
                    name: "http.server.request.duration".to_owned().into(),
                    instrument: InstrumentSpec::Histogram,
                    unit: "s".to_owned(),
                    attributes: vec![],
                    entity_associations: vec![],
                    common: CommonFields {
                        brief: "Duration of HTTP server requests".to_owned(),
                        note: "".to_owned(),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                }],
                spans: vec![Span {
                    r#type: "http.client".to_owned().into(),
                    kind: SpanKindSpec::Client,
                    name: SpanName {
                        note: "HTTP client span".to_owned(),
                    },
                    attributes: vec![],
                    entity_associations: vec![],
                    common: CommonFields {
                        brief: "HTTP client span".to_owned(),
                        note: "".to_owned(),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                }],
                events: vec![Event {
                    name: "exception".to_owned().into(),
                    attributes: vec![],
                    entity_associations: vec![],
                    common: CommonFields {
                        brief: "An exception event".to_owned(),
                        note: "".to_owned(),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                }],
                entities: vec![Entity {
                    r#type: "service".to_owned().into(),
                    identity: vec![],
                    description: vec![],
                    common: CommonFields {
                        brief: "A service entity".to_owned(),
                        note: "".to_owned(),
                        stability: Stability::Stable,
                        deprecated: None,
                        annotations: BTreeMap::new(),
                    },
                }],
            },
            refinements: Refinements {
                metrics: vec![],
                spans: vec![],
                events: vec![],
            },
        }
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

    // =========================================================================
    // SearchContext Tests
    // =========================================================================

    #[test]
    fn test_from_registry_indexes_all_types() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        // Check attributes are indexed
        assert!(ctx.get_attribute("http.request.method").is_some());
        assert!(ctx.get_attribute("http.response.status_code").is_some());
        assert!(ctx.get_attribute("db.system").is_some());

        // Check metric is indexed
        assert!(ctx.get_metric("http.server.request.duration").is_some());

        // Check span is indexed
        assert!(ctx.get_span("http.client").is_some());

        // Check event is indexed
        assert!(ctx.get_event("exception").is_some());

        // Check entity is indexed
        assert!(ctx.get_entity("service").is_some());
    }

    #[test]
    fn test_get_attribute_not_found() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        assert!(ctx.get_attribute("nonexistent.attribute").is_none());
        assert!(ctx.get_metric("nonexistent.metric").is_none());
        assert!(ctx.get_span("nonexistent.span").is_none());
        assert!(ctx.get_event("nonexistent.event").is_none());
        assert!(ctx.get_entity("nonexistent.entity").is_none());
    }

    #[test]
    fn test_search_with_query_returns_matches() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        let (results, total) = ctx.search(Some("http"), SearchType::All, None, 10, 0);

        // Should find http.request.method, http.response.status_code,
        // http.server.request.duration, http.client
        assert!(total >= 4);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_browse_mode() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        // None query = browse mode
        let (results, total) = ctx.search(None, SearchType::All, None, 100, 0);

        // Should return all items: 5 attributes + 1 metric + 1 span + 1 event + 1 entity = 9
        assert_eq!(total, 9);
        assert_eq!(results.len(), 9);
    }

    #[test]
    fn test_search_type_filter() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        // Filter by Attribute only
        let (results, total) = ctx.search(None, SearchType::Attribute, None, 100, 0);
        assert_eq!(total, 5); // 5 attributes (3 regular + 1 template + 1 development)
        assert_eq!(results.len(), 5);

        // Filter by Metric only
        let (results, total) = ctx.search(None, SearchType::Metric, None, 100, 0);
        assert_eq!(total, 1);
        assert_eq!(results.len(), 1);

        // Filter by Span only
        let (_, total) = ctx.search(None, SearchType::Span, None, 100, 0);
        assert_eq!(total, 1);

        // Filter by Event only
        let (_, total) = ctx.search(None, SearchType::Event, None, 100, 0);
        assert_eq!(total, 1);

        // Filter by Entity only
        let (_, total) = ctx.search(None, SearchType::Entity, None, 100, 0);
        assert_eq!(total, 1);
    }

    #[test]
    fn test_search_pagination() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        // Get first 2 items
        let (results1, total1) = ctx.search(None, SearchType::All, None, 2, 0);
        assert_eq!(total1, 9);
        assert_eq!(results1.len(), 2);

        // Get next 2 items with offset
        let (results2, total2) = ctx.search(None, SearchType::All, None, 2, 2);
        assert_eq!(total2, 9);
        assert_eq!(results2.len(), 2);

        // Get remaining items
        let (results3, _) = ctx.search(None, SearchType::All, None, 100, 4);
        assert_eq!(results3.len(), 5);
    }

    #[test]
    fn test_search_limit_capped_at_200() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        // Request limit > 200 should be capped
        let (results, _) = ctx.search(None, SearchType::All, None, 500, 0);

        // We only have 9 items, so we get 9 (not testing the cap directly,
        // but ensuring it doesn't crash with large limit)
        assert_eq!(results.len(), 9);
    }

    #[test]
    fn test_search_no_results() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        let (results, total) = ctx.search(Some("zzzznonexistent"), SearchType::All, None, 10, 0);

        assert_eq!(total, 0);
        assert!(results.is_empty());
    }

    // =========================================================================
    // Template Attribute Tests
    // =========================================================================

    #[test]
    fn test_get_template_found() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        let result = ctx.get_template("test.template");
        assert!(result.is_some());
        assert_eq!(result.unwrap().key, "test.template");
    }

    #[test]
    fn test_get_template_not_found() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        // Regular attribute should not be found via get_template
        assert!(ctx.get_template("http.request.method").is_none());
        // Nonexistent should not be found
        assert!(ctx.get_template("nonexistent").is_none());
    }

    #[test]
    fn test_find_template_exact_match() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        let result = ctx.find_template("test.template");
        assert!(result.is_some());
        assert_eq!(result.unwrap().key, "test.template");
    }

    #[test]
    fn test_find_template_prefix_match() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        // find_template should find templates by prefix
        let result = ctx.find_template("test.template.foo");
        assert!(result.is_some());
        assert_eq!(result.unwrap().key, "test.template");
    }

    #[test]
    fn test_find_template_not_found() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        assert!(ctx.find_template("nonexistent.template").is_none());
    }

    // =========================================================================
    // Stability Filtering Tests
    // =========================================================================

    #[test]
    fn test_search_stability_filter_stable() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        // Filter by Stable only
        let (results, total) =
            ctx.search(None, SearchType::Attribute, Some(Stability::Stable), 100, 0);

        // Should return only stable attributes (4: http.request.method, http.response.status_code, db.system, test.template)
        assert_eq!(total, 4);
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_search_stability_filter_development() {
        let registry = make_test_registry();
        let ctx = SearchContext::from_registry(&registry);

        // Filter by Development only
        let (results, total) = ctx.search(
            None,
            SearchType::Attribute,
            Some(Stability::Development),
            100,
            0,
        );

        // Should return only development attributes (1: experimental.feature)
        assert_eq!(total, 1);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_searchable_item_stability() {
        let attr = make_attribute("test", "test", "", false);
        let item = SearchableItem::Attribute(Arc::new(attr));

        assert_eq!(item.stability(), &Stability::Stable);

        let dev_attr = make_development_attribute("dev", "dev");
        let dev_item = SearchableItem::Attribute(Arc::new(dev_attr));

        assert_eq!(dev_item.stability(), &Stability::Development);
    }
}
