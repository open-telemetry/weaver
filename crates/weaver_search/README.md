# weaver_search

Search functionality for OpenTelemetry semantic convention registries.

This crate provides `SearchContext`, a search engine for querying resolved
registries with support for:

- Fuzzy text matching
- Type filtering (attributes, metrics, spans, events, entities)
- Stability filtering
- Relevance scoring
- Pagination
