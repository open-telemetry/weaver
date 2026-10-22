//! Signal definitions and attribute catalog extracted from a resolved V1 schema.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use weaver_resolved_schema::v1::ResolvedTelemetrySchema;
use weaver_semconv::v1::attribute::{AttributeType, BasicRequirementLevelSpec, RequirementLevel};
use weaver_semconv::v1::group::GroupType;
use weaver_semconv::v1::stability::Stability;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sig {
    Span,
    Metric,
    Event,
    Entity,
}

impl Sig {
    pub fn plural(self) -> &'static str {
        match self {
            Sig::Span => "spans",
            Sig::Metric => "metrics",
            Sig::Event => "events",
            Sig::Entity => "entities",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Required,
    CondRequired,
    Recommended,
    OptIn,
}

impl Level {
    pub fn counts_for_strict(self) -> bool {
        self != Level::OptIn
    }
}

impl From<&RequirementLevel> for Level {
    fn from(r: &RequirementLevel) -> Self {
        match r {
            RequirementLevel::Basic(BasicRequirementLevelSpec::Required) => Level::Required,
            RequirementLevel::Basic(BasicRequirementLevelSpec::Recommended)
            | RequirementLevel::Recommended { .. } => Level::Recommended,
            RequirementLevel::Basic(BasicRequirementLevelSpec::OptIn)
            | RequirementLevel::OptIn { .. } => Level::OptIn,
            RequirementLevel::ConditionallyRequired { .. } => Level::CondRequired,
        }
    }
}

/// A signal definition reduced to what schema-mode policy application needs.
#[derive(Debug, Clone)]
pub struct Def {
    pub sig: Sig,
    pub id: String,
    /// Metric name, event name, entity name, or the group id for spans.
    pub name: String,
    pub span_kind: Option<String>,
    pub instrument: Option<String>,
    pub unit: Option<String>,
    pub attrs: BTreeMap<String, Level>,
    pub stable: bool,
    pub deprecated: bool,
    pub extends: Option<String>,
    /// False when the registry lists no attributes for the signal.
    pub attrs_listed: bool,
    /// System attribute key and value implied by a per-system span (`span.db.cassandra.client`).
    pub system: Option<(String, String)>,
    /// Per-system variant synthesized from a generic span; a source only.
    pub synthetic: bool,
}

#[derive(Debug, Clone)]
pub struct AttrInfo {
    pub deprecated: bool,
    pub template: bool,
    pub ty: AttributeType,
}

/// Defs and attribute catalog of one registry version.
pub struct Registry {
    pub label: String,
    pub defs: Vec<Def>,
    pub attrs: HashMap<String, AttrInfo>,
    pub metric_names: BTreeSet<String>,
    pub event_names: BTreeSet<String>,
}

#[allow(deprecated)]
fn is_deprecated(stability: Option<&Stability>, deprecated: bool) -> bool {
    deprecated || matches!(stability, Some(Stability::Deprecated))
}

/// Namespaces whose per-system spans imply a `<ns>.system[.name]` value.
const SYSTEM_NAMESPACES: [&str; 2] = ["db", "rpc"];

fn enum_member(ty: &AttributeType, seg: &str) -> Option<String> {
    let AttributeType::Enum { members } = ty else {
        return None;
    };
    members.iter().find_map(|m| {
        let v = m.value.to_string();
        let hit = v == seg
            || m.id == seg
            || v.replace('.', "") == seg
            || v.replace('.', "_") == seg
            || v.ends_with(&format!(".{seg}"))
            || v.replace(['.', '_'], "") == seg.replace('_', "");
        hit.then_some(v)
    })
}

/// Sets `system` from the group id (or `extends`) and adds the system attribute when unlisted.
fn derive_system(d: &mut Def, attrs: &HashMap<String, AttrInfo>) {
    let mut implied: Option<(String, [String; 2])> = None;
    for id in std::iter::once(d.id.clone()).chain(d.extends.clone()) {
        let stem = id
            .strip_prefix("span.")
            .or_else(|| id.strip_prefix("trace."))
            .unwrap_or(&id);
        let mut parts = stem.split('.');
        let (Some(ns), Some(seg)) = (parts.next(), parts.next()) else {
            continue;
        };
        if !SYSTEM_NAMESPACES.contains(&ns) {
            continue;
        }
        let keys = [format!("{ns}.system.name"), format!("{ns}.system")];
        let key = keys
            .iter()
            .find(|k| attrs.get(*k).is_some_and(|a| !a.deprecated))
            .or_else(|| keys.iter().find(|k| attrs.contains_key(*k)));
        let Some(key) = key else { continue };
        implied.get_or_insert_with(|| (key.clone(), keys.clone()));
        let Some(value) = enum_member(&attrs[key].ty, seg) else {
            continue;
        };
        if !keys.iter().any(|k| d.attrs.contains_key(k)) {
            _ = d.attrs.insert(key.clone(), Level::Required);
        }
        let key = keys
            .iter()
            .find(|k| d.attrs.contains_key(*k))
            .cloned()
            .unwrap_or_else(|| key.clone());
        d.system = Some((key, value));
        return;
    }
    // Every DB/RPC span carries the system attribute; class spans (`span.db.sql.client`)
    // only imply it.
    if let Some((key, keys)) = implied {
        if !keys.iter().any(|k| d.attrs.contains_key(k)) {
            _ = d.attrs.insert(key, Level::Required);
        }
    }
}

impl Registry {
    pub fn from_schema(label: &str, schema: &ResolvedTelemetrySchema) -> Self {
        let catalog = schema.catalog();
        let info = |a: &weaver_resolved_schema::v1::attribute::Attribute| AttrInfo {
            deprecated: is_deprecated(a.stability.as_ref(), a.deprecated.is_some()),
            template: matches!(a.r#type, AttributeType::Template(_)),
            ty: a.r#type.clone(),
        };
        // Main-registry definitions win over dependency ones with the same key.
        let mut attrs: HashMap<String, AttrInfo> = HashMap::new();
        for g in schema.registry.groups.iter().filter(|g| {
            g.lineage
                .as_ref()
                .map(|l| l.provenance().schema_url.as_str() == schema.schema_url.as_str())
                .unwrap_or(true)
        }) {
            for a in g.attributes.iter().filter_map(|r| catalog.attribute(r)) {
                _ = attrs.insert(a.name.clone(), info(a));
            }
        }
        for a in catalog.attributes() {
            _ = attrs.entry(a.name.clone()).or_insert_with(|| info(a));
        }

        let mut by_key: BTreeMap<(Sig, String), (bool, Def)> = BTreeMap::new();
        for g in &schema.registry.groups {
            let (sig, name) = match g.r#type {
                GroupType::Span => (Sig::Span, g.id.clone()),
                GroupType::Metric => match &g.metric_name {
                    Some(n) => (Sig::Metric, n.clone()),
                    None => continue,
                },
                GroupType::Event => (Sig::Event, g.name.clone().unwrap_or_else(|| g.id.clone())),
                GroupType::Entity => (Sig::Entity, g.name.clone().unwrap_or_else(|| g.id.clone())),
                _ => continue,
            };
            let attrs: BTreeMap<String, Level> = g
                .attributes
                .iter()
                .filter_map(|r| catalog.attribute(r))
                .map(|a| (a.name.clone(), Level::from(&a.requirement_level)))
                .collect();
            let from_main = g
                .lineage
                .as_ref()
                .map(|l| l.provenance().schema_url.as_str() == schema.schema_url.as_str())
                .unwrap_or(true);
            let def = Def {
                sig,
                id: g.id.clone(),
                name: name.clone(),
                span_kind: g
                    .span_kind
                    .as_ref()
                    .map(|k| format!("{k:?}").to_uppercase()),
                instrument: g
                    .instrument
                    .as_ref()
                    .map(|i| format!("{i:?}").to_lowercase()),
                unit: g.unit.clone(),
                attrs_listed: !attrs.is_empty(),
                attrs,
                stable: matches!(g.stability, Some(Stability::Stable)),
                deprecated: is_deprecated(g.stability.as_ref(), g.deprecated.is_some()),
                extends: g.extends.clone(),
                system: None,
                synthetic: false,
            };
            let key = (sig, name);
            // Dependency groups lose to main-registry groups with the same name.
            match by_key.get(&key) {
                Some((true, _)) if !from_main => {}
                _ => {
                    _ = by_key.insert(key, (from_main, def));
                }
            }
        }
        let mut defs: Vec<Def> = by_key.into_values().map(|(_, d)| d).collect();
        for d in defs.iter_mut().filter(|d| d.sig == Sig::Span) {
            derive_system(d, &attrs);
        }
        // `<ns>.<system>.attributes` groups (1.37 RPC) hold per-system span attributes that no
        // span references; they become per-system variants of the generic spans.
        let mut variants = vec![];
        for g in schema
            .registry
            .groups
            .iter()
            .filter(|g| g.r#type == GroupType::AttributeGroup)
        {
            let mut parts = g.id.split('.');
            let (Some(ns), Some(sys), Some("attributes"), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            if !SYSTEM_NAMESPACES.contains(&ns) {
                continue;
            }
            let extra: Vec<(String, Level)> = g
                .attributes
                .iter()
                .filter_map(|r| catalog.attribute(r))
                .map(|a| (a.name.clone(), Level::from(&a.requirement_level)))
                .collect();
            for d in defs.iter().filter(|d| {
                d.sig == Sig::Span && d.system.is_none() && d.id.split('.').any(|p| p == ns)
            }) {
                let Some(key) = [format!("{ns}.system.name"), format!("{ns}.system")]
                    .into_iter()
                    .find(|k| d.attrs.contains_key(k))
                else {
                    continue;
                };
                // The group id names the system even when the enum (open) doesn't list it.
                let value = attrs
                    .get(&key)
                    .and_then(|a| enum_member(&a.ty, sys))
                    .unwrap_or_else(|| sys.to_owned());
                let mut v = d.clone();
                v.name = format!("{}[{value}]", d.name);
                v.id = v.name.clone();
                v.attrs.extend(extra.iter().cloned());
                v.system = Some((key, value));
                v.synthetic = true;
                variants.push(v);
            }
        }
        defs.extend(variants);
        let metric_names = defs
            .iter()
            .filter(|d| d.sig == Sig::Metric)
            .map(|d| d.name.clone())
            .collect();
        let event_names = defs
            .iter()
            .filter(|d| d.sig == Sig::Event)
            .map(|d| d.name.clone())
            .collect();
        Self {
            label: label.to_owned(),
            defs,
            attrs,
            metric_names,
            event_names,
        }
    }

    /// Exact key, or a key under a template attribute (`http.request.header.foo`).
    pub fn has_attr(&self, key: &str) -> bool {
        let key = key.trim_end_matches('.');
        if self.attrs.contains_key(key) {
            return true;
        }
        let mut k = key;
        while let Some(i) = k.rfind('.') {
            k = &k[..i];
            if self.attrs.get(k).is_some_and(|a| a.template) {
                return true;
            }
        }
        false
    }

    pub fn attr_deprecated(&self, key: &str) -> bool {
        self.attrs
            .get(key.trim_end_matches('.'))
            .is_some_and(|a| a.deprecated)
    }
}
