// SPDX-License-Identifier: Apache-2.0

//! Resolution of base V2 signals: metrics, spans, events, and entities.

use std::collections::{BTreeMap, HashSet};
use weaver_common::error::handle_errors;
use weaver_resolved_schema::v2::attribute::Attribute;
use weaver_resolved_schema::v2::entity::EntityAssociation as ResolvedEntityAssociation;
use weaver_resolved_schema::v2::provenance::Provenance;
use weaver_semconv::provenance::Provenance as SemconvProvenance;
use weaver_semconv::schema_url::SchemaUrl;
use weaver_semconv::v2::attribute::{AttributeRef, RequirementLevel};
use weaver_semconv::v2::entity::IdentityAttributeRef;
use weaver_semconv::v2::entity_association::EntityAssociation as SemconvEntityAssociation;
use weaver_semconv::v2::metric::InstrumentSpec;
use weaver_semconv::v2::signal_id::SignalId;
use weaver_semconv::v2::signal_requirement_level::SignalRequirementLevel;
use weaver_semconv::v2::span::{SpanKindSpec, SpanName};
use weaver_semconv::v2::CommonFields;

use super::attributes::Catalog;
use super::dependency::V2Dependencies;
use super::groups::{ExpandKey, ExpandedGroups, GroupAttributeItem};
use super::index::Index;
use super::rules;
use crate::error::Error;
use crate::graph::chosen_versions::SchemaCacheLookup;
use crate::graph::exclusion::is_excluded;

/// Typed discriminator for the four V2 signal kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SignalKind {
    Metric,
    Span,
    Event,
    Entity,
}

impl SignalKind {
    /// All four signal kinds in canonical order.
    pub const ALL: [Self; 4] = [Self::Metric, Self::Span, Self::Event, Self::Entity];

    /// Lowercase signal kind identifier (`"metric"`, `"span"`, `"event"`, `"entity"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Span => "span",
            Self::Event => "event",
            Self::Entity => "entity",
        }
    }

    /// Title-case signal kind label used in diagnostic messages.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Metric => "Metric",
            Self::Span => "Span",
            Self::Event => "Event",
            Self::Entity => "Entity",
        }
    }

    /// Optional leading `<kind>.` prefix stripped when normalizing refinement IDs.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Metric => "metric.",
            Self::Span => "span.",
            Self::Event => "event.",
            Self::Entity => "entity.",
        }
    }

    /// Strips an optional leading `<kind>.` prefix from `id`.
    #[must_use]
    pub fn normalize_id(self, id: &str) -> SignalId {
        SignalId::from(id.strip_prefix(self.prefix()).unwrap_or(id))
    }
}

/// Converts an [`IdentityAttributeRef`] into an [`AttributeRef`] so identity references share override resolution.
#[must_use]
pub(crate) fn entity_identity_to_attr_ref(id_ref: &IdentityAttributeRef) -> AttributeRef {
    AttributeRef {
        r#ref: id_ref.r#ref.clone(),
        brief: id_ref.brief.clone(),
        examples: id_ref.examples.clone(),
        requirement_level: None,
        note: id_ref.note.clone(),
        annotations: id_ref.annotations.clone(),
    }
}

/// Unresolved or resolved entity associations on a signal.
#[derive(Debug, Clone)]
pub(crate) enum EntityAssociations {
    Raw(Vec<SemconvEntityAssociation>),
    Resolved(Vec<ResolvedEntityAssociation>),
}

impl Default for EntityAssociations {
    fn default() -> Self {
        EntityAssociations::Raw(Vec::new())
    }
}

impl EntityAssociations {
    /// Returns `true` if there are no entity associations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            EntityAssociations::Raw(v) => v.is_empty(),
            EntityAssociations::Resolved(v) => v.is_empty(),
        }
    }
}

/// Resolved metric signal with expanded attributes prior to final emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedMetric {
    pub name: SignalId,
    pub instrument: InstrumentSpec,
    pub unit: String,
    pub attributes: Vec<(Attribute, RequirementLevel)>,
    pub entity_associations: EntityAssociations,
    pub association_origins: BTreeMap<String, SchemaUrl>,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub common: CommonFields,
    pub provenance: Provenance,
}

/// Resolved span link with catalog-resolved attributes prior to final emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedSpanLink {
    pub r#ref: SignalId,
    pub brief: Option<String>,
    pub note: Option<String>,
    pub attributes: Vec<(Attribute, RequirementLevel)>,
}

/// Resolved span signal with expanded attributes prior to final emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedSpan {
    pub r#type: SignalId,
    pub kind: SpanKindSpec,
    pub name: SpanName,
    pub attributes: Vec<(Attribute, RequirementLevel, Option<bool>)>,
    pub links: Vec<ResolvedSpanLink>,
    pub entity_associations: EntityAssociations,
    pub association_origins: BTreeMap<String, SchemaUrl>,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub common: CommonFields,
    pub provenance: Provenance,
}

/// Resolved event signal with expanded attributes prior to final emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedEvent {
    pub name: SignalId,
    pub attributes: Vec<(Attribute, RequirementLevel)>,
    pub entity_associations: EntityAssociations,
    pub association_origins: BTreeMap<String, SchemaUrl>,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub common: CommonFields,
    pub provenance: Provenance,
}

/// Resolved entity definition with identity and description attributes prior to final emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedEntity {
    pub r#type: SignalId,
    pub identity: Vec<Attribute>,
    pub description: Vec<(Attribute, RequirementLevel)>,
    pub requirement_level: Option<SignalRequirementLevel>,
    pub common: CommonFields,
    pub provenance: Provenance,
}

/// All resolved local base signals in a registry.
#[derive(Debug, Default)]
pub(crate) struct ResolvedSignals {
    pub metrics: Vec<ResolvedMetric>,
    pub spans: Vec<ResolvedSpan>,
    pub events: Vec<ResolvedEvent>,
    pub entities: Vec<ResolvedEntity>,
}

/// Resolves expanded group attribute items into sorted `(Attribute, RequirementLevel)` pairs.
fn resolve_expanded_attributes<C: SchemaCacheLookup>(
    expanded_items: &[GroupAttributeItem],
    used_in: &str,
    consumer_excluded: bool,
    provenance: &SemconvProvenance,
    catalog: &mut Catalog<'_, C>,
) -> Result<Vec<(Attribute, RequirementLevel)>, Error> {
    let mut attributes = Vec::with_capacity(expanded_items.len());
    for item in expanded_items {
        let resolved_attr =
            catalog.resolve_group_item(item, used_in, consumer_excluded, Some(provenance))?;
        let req_level = rules::resolve_requirement_level(&item.aref);
        attributes.push((resolved_attr, req_level));
    }
    attributes.sort_by(|(a, _), (b, _)| a.key.cmp(&b.key));
    Ok(attributes)
}

/// Checks that every link on a locally defined span targets a local, imported, or dependency span type.
///
/// Imported spans are not checked: their targets may live in a transitive dependency.
pub(crate) fn validate_span_link_targets(
    index: &Index,
    signals: &ResolvedSignals,
    imported_span_types: impl IntoIterator<Item = SignalId>,
    deps: &V2Dependencies,
) -> Result<(), Error> {
    let imported: HashSet<SignalId> = imported_span_types.into_iter().collect();
    // Base spans only: refinement ids are not link targets.
    let mut dep_spans: Option<HashSet<SignalId>> = None;
    let mut errors = Vec::new();
    for span in &signals.spans {
        let provenance = || {
            index
                .spans()
                .get(&span.r#type)
                .map(|s| Box::new(s.provenance.clone()))
        };
        for link in &span.links {
            if let Some(target) = index.spans().get(&link.r#ref) {
                if is_excluded(&target.item.common.annotations)
                    && !is_excluded(&span.common.annotations)
                {
                    errors.push(Error::ExcludedFromDependencyResolution {
                        id: link.r#ref.to_string(),
                        r#type: SignalKind::Span.as_str().to_owned(),
                        used_in: span.r#type.to_string(),
                        provenance: provenance(),
                    });
                }
                continue;
            }
            if imported.contains(&link.r#ref) {
                continue;
            }
            let dep_spans = dep_spans.get_or_insert_with(|| {
                deps.adapters()
                    .iter()
                    .flat_map(|d| d.importable_spans())
                    .filter(|s| !s.excluded)
                    .map(|s| s.r#type)
                    .collect()
            });
            if !dep_spans.contains(&link.r#ref) {
                errors.push(Error::SpanLinkTargetNotFound {
                    span_id: span.r#type.to_string(),
                    link_ref: link.r#ref.to_string(),
                    provenance: provenance(),
                });
            }
        }
    }
    handle_errors(errors)
}

/// Resolves all base signals in the index against expanded groups and catalog.
pub(crate) fn resolve_bases<'a, C: SchemaCacheLookup>(
    index: &Index,
    groups: &ExpandedGroups,
    catalog: &mut Catalog<'a, C>,
) -> Result<ResolvedSignals, Error> {
    let mut validation_errors = Vec::new();

    let mut metrics = Vec::with_capacity(index.metrics().len());
    for (name, indexed_m) in index.metrics() {
        let m = &indexed_m.item;
        let provenance = Provenance {
            source: None,
            path: indexed_m.provenance.path.clone(),
        };
        let used_in = format!("metric.{name}");
        let attributes = resolve_expanded_attributes(
            groups.items_for(&ExpandKey::Metric(name.clone())),
            &used_in,
            is_excluded(&m.common.annotations),
            &indexed_m.provenance,
            catalog,
        )?;

        metrics.push(ResolvedMetric {
            name: name.clone(),
            instrument: m.instrument,
            unit: m.unit.clone(),
            attributes,
            entity_associations: EntityAssociations::Raw(m.entity_associations.clone()),
            association_origins: BTreeMap::new(),
            requirement_level: m.requirement_level.clone(),
            common: m.common.clone(),
            provenance,
        });
    }
    metrics.sort_by(|a, b| a.name.cmp(&b.name));

    let mut spans = Vec::with_capacity(index.spans().len());
    for (type_id, indexed_s) in index.spans() {
        let s = &indexed_s.item;
        let provenance = Provenance {
            source: None,
            path: indexed_s.provenance.path.clone(),
        };
        let s_excluded = is_excluded(&s.common.annotations);
        let used_in = format!("span.{type_id}");

        let expanded_items = groups.items_for(&ExpandKey::Span(type_id.clone()));

        let mut attributes = Vec::with_capacity(expanded_items.len());
        let mut span_attr_keys = HashSet::with_capacity(expanded_items.len());

        for item in expanded_items {
            let resolved_attr = catalog.resolve_group_item(
                item,
                &used_in,
                s_excluded,
                Some(&indexed_s.provenance),
            )?;
            _ = span_attr_keys.insert(resolved_attr.key.clone());
            let req_level = rules::resolve_requirement_level(&item.aref);
            attributes.push((resolved_attr, req_level, item.sampling_relevant));
        }
        attributes.sort_by(|(a, _, _), (b, _, _)| a.key.cmp(&b.key));

        for template in &s.name.templates {
            for attr_key in &template.attributes {
                if !span_attr_keys.contains(attr_key) {
                    validation_errors.push(Error::SpanNameAttributeNotOnSpan {
                        span_id: type_id.to_string(),
                        attribute_key: attr_key.clone(),
                        provenance: Some(Box::new(indexed_s.provenance.clone())),
                    });
                }
            }
        }

        let mut links = Vec::with_capacity(s.links.len());
        for link in &s.links {
            let mut link_attributes = Vec::with_capacity(link.attributes.len());
            for la in &link.attributes {
                let resolved_attr = catalog.resolve_attribute_ref(
                    &la.base,
                    &used_in,
                    s_excluded,
                    Some(&indexed_s.provenance),
                )?;
                link_attributes.push((resolved_attr, rules::resolve_requirement_level(&la.base)));
            }
            links.push(ResolvedSpanLink {
                r#ref: link.r#ref.clone(),
                brief: link.brief.clone(),
                note: link.note.clone(),
                attributes: link_attributes,
            });
        }

        spans.push(ResolvedSpan {
            r#type: type_id.clone(),
            kind: s.kind,
            name: s.name.clone(),
            attributes,
            links,
            entity_associations: EntityAssociations::Raw(s.entity_associations.clone()),
            association_origins: BTreeMap::new(),
            requirement_level: s.requirement_level.clone(),
            common: s.common.clone(),
            provenance,
        });
    }
    spans.sort_by(|a, b| a.r#type.cmp(&b.r#type));

    let mut events = Vec::with_capacity(index.events().len());
    for (name, indexed_e) in index.events() {
        let e = &indexed_e.item;
        let provenance = Provenance {
            source: None,
            path: indexed_e.provenance.path.clone(),
        };
        let used_in = format!("event.{name}");
        let attributes = resolve_expanded_attributes(
            groups.items_for(&ExpandKey::Event(name.clone())),
            &used_in,
            is_excluded(&e.common.annotations),
            &indexed_e.provenance,
            catalog,
        )?;

        events.push(ResolvedEvent {
            name: name.clone(),
            attributes,
            entity_associations: EntityAssociations::Raw(e.entity_associations.clone()),
            association_origins: BTreeMap::new(),
            requirement_level: e.requirement_level.clone(),
            common: e.common.clone(),
            provenance,
        });
    }
    events.sort_by(|a, b| a.name.cmp(&b.name));

    let mut entities = Vec::with_capacity(index.entities().len());
    for (type_id, indexed_en) in index.entities() {
        let en = &indexed_en.item;
        let provenance = Provenance {
            source: None,
            path: indexed_en.provenance.path.clone(),
        };
        let en_excluded = is_excluded(&en.common.annotations);
        let used_in = format!("entity.{type_id}");

        if en.identity.is_empty() {
            validation_errors.push(Error::EntityMissingIdentity {
                entity_type: type_id.to_string(),
                provenance: Some(Box::new(indexed_en.provenance.clone())),
            });
        }

        let mut identity = Vec::with_capacity(en.identity.len());
        for id_ref in &en.identity {
            let aref = entity_identity_to_attr_ref(id_ref);
            let resolved_attr = catalog.resolve_attribute_ref(
                &aref,
                &used_in,
                en_excluded,
                Some(&indexed_en.provenance),
            )?;
            identity.push(resolved_attr);
        }
        identity.sort_by(|a, b| a.key.cmp(&b.key));

        let mut description = Vec::with_capacity(en.description.len());
        for desc_ref in &en.description {
            let resolved_attr = catalog.resolve_attribute_ref(
                desc_ref,
                &used_in,
                en_excluded,
                Some(&indexed_en.provenance),
            )?;
            let req_level = rules::resolve_requirement_level(desc_ref);
            description.push((resolved_attr, req_level));
        }
        description.sort_by(|(a, _), (b, _)| a.key.cmp(&b.key));

        entities.push(ResolvedEntity {
            r#type: type_id.clone(),
            identity,
            description,
            requirement_level: en.requirement_level.clone(),
            common: en.common.clone(),
            provenance,
        });
    }
    entities.sort_by(|a, b| a.r#type.cmp(&b.r#type));

    handle_errors(validation_errors)?;

    Ok(ResolvedSignals {
        metrics,
        spans,
        events,
        entities,
    })
}
