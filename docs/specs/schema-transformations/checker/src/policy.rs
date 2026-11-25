//! Transformation file parsing (proto#865-style policies, schema-mode subset).

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Context, Result};
use serde_yaml::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    Span,
    Datapoint,
    Log,
    Resource,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    Attr(Scope, String),
    MetricName,
    EventName,
    SpanKind,
    /// Any other record / datapoint field (span name, status, datapoint value).
    Other(String),
}

impl Field {
    pub fn attr_key(&self) -> Option<(Scope, &str)> {
        match self {
            Field::Attr(s, k) => Some((*s, k)),
            _ => None,
        }
    }
}

impl std::fmt::Display for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Field::Attr(_, k) => write!(f, "{k}"),
            Field::MetricName => write!(f, "<metric name>"),
            Field::EventName => write!(f, "<event name>"),
            Field::SpanKind => write!(f, "<span kind>"),
            Field::Other(o) => write!(f, "<{o}>"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Trace,
    Metric,
    Log,
    Entity,
    MetricFilter,
}

impl Kind {
    pub fn scope(self) -> Scope {
        match self {
            Kind::Trace => Scope::Span,
            Kind::Metric | Kind::MetricFilter => Scope::Datapoint,
            Kind::Log => Scope::Log,
            Kind::Entity => Scope::Resource,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pred {
    Exists,
    Equals(String),
    Regex(String),
    Cmp(String, f64),
}

#[derive(Debug, Clone)]
pub struct Matcher {
    pub field: Field,
    pub pred: Pred,
    pub negate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Add {
    pub target: Field,
    pub value: Option<String>,
    pub from: Vec<Field>,
    pub separator: Option<String>,
    pub upsert: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rename {
    pub from: Field,
    pub to: Field,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenamePrefix {
    pub from: Field,
    pub to: String,
    pub replace: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Split {
    pub from: Field,
    pub separator: String,
    pub to: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Map {
    pub target: Field,
    pub values: Vec<(String, String)>,
    pub unmapped: String,
}

#[derive(Debug, Clone, Default)]
pub struct Ops {
    pub remove: Vec<Field>,
    pub rename: Vec<Rename>,
    pub rename_prefix: Vec<RenamePrefix>,
    pub split: Vec<Split>,
    pub add: Vec<Add>,
    pub map: Vec<Map>,
    pub convert: Vec<(Field, String)>,
    pub discard: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Policy {
    pub id: String,
    pub kind: Kind,
    pub matchers: Vec<Matcher>,
    pub ops: Ops,
}

#[derive(Debug, Clone, Default)]
pub struct TransformFile {
    pub source_url: String,
    pub target_url: String,
    /// `registry_path` overrides, relative to the transformation file.
    pub source_path: Option<String>,
    pub target_path: Option<String>,
    pub gaps: Option<Value>,
    pub policies: Vec<Policy>,
    pub parse_errors: Vec<String>,
}

pub fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn get<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    v.as_mapping().and_then(|m| m.get(k))
}

fn parse_field(v: &Value) -> Result<Field> {
    let m = v
        .as_mapping()
        .ok_or_else(|| anyhow!("field is not a mapping: {v:?}"))?;
    let (k, val) = m.iter().next().ok_or_else(|| anyhow!("empty field"))?;
    let k = k.as_str().unwrap_or_default();
    let attr_key = || -> Result<String> {
        val.as_sequence()
            .and_then(|s| s.first())
            .and_then(scalar)
            .ok_or_else(|| anyhow!("attribute path expected for {k}"))
    };
    Ok(match k {
        "span_attribute" => Field::Attr(Scope::Span, attr_key()?),
        "datapoint_attribute" => Field::Attr(Scope::Datapoint, attr_key()?),
        "log_attribute" => Field::Attr(Scope::Log, attr_key()?),
        "resource_attribute" => Field::Attr(Scope::Resource, attr_key()?),
        "descriptor_field" | "record_field" | "datapoint_field" => {
            let f = val.as_str().unwrap_or_default();
            match f {
                "METRIC_DESCRIPTOR_FIELD_NAME" => Field::MetricName,
                "LOG_RECORD_FIELD_EVENT_NAME" => Field::EventName,
                "SPAN_RECORD_FIELD_KIND" => Field::SpanKind,
                other => Field::Other(other.to_owned()),
            }
        }
        other => bail!("unknown field kind {other}"),
    })
}

fn parse_matcher(v: &Value) -> Result<Matcher> {
    let field = parse_field(get(v, "target").ok_or_else(|| anyhow!("matcher without target"))?)?;
    let p = get(v, "predicate").ok_or_else(|| anyhow!("matcher without predicate"))?;
    let pm = p
        .as_mapping()
        .ok_or_else(|| anyhow!("predicate not a mapping"))?;
    let (pk, pv) = pm.iter().next().ok_or_else(|| anyhow!("empty predicate"))?;
    let pred = match pk.as_str().unwrap_or_default() {
        "exists" => Pred::Exists,
        "equals" => Pred::Equals(scalar(pv).unwrap_or_default()),
        "regex" => Pred::Regex(scalar(pv).unwrap_or_default()),
        op @ ("gt" | "gte" | "lt" | "lte") => Pred::Cmp(
            op.to_owned(),
            scalar(pv).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        ),
        other => bail!("unknown predicate {other}"),
    };
    let negate = get(v, "negate").and_then(Value::as_bool).unwrap_or(false);
    Ok(Matcher {
        field,
        pred,
        negate,
    })
}

/// An attribute key written by `to` gets the scope of the field it comes from.
fn key_field(scope_of: &Field, key: &str, default: Scope) -> Field {
    let scope = scope_of.attr_key().map(|(s, _)| s).unwrap_or(default);
    Field::Attr(scope, key.to_owned())
}

fn seq(v: &Value, k: &str) -> Vec<Value> {
    get(v, k)
        .and_then(Value::as_sequence)
        .cloned()
        .unwrap_or_default()
}

fn str_pairs(v: Option<&Value>) -> Vec<(String, String)> {
    v.and_then(Value::as_mapping)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((scalar(k)?, scalar(v)?)))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_ops(body: &Value, scope: Scope) -> Result<Ops> {
    let mut ops = Ops::default();
    for e in seq(body, "remove") {
        ops.remove
            .push(parse_field(get(&e, "target").context("remove.target")?)?);
    }
    for e in seq(body, "discard") {
        ops.discard
            .push(parse_field(get(&e, "target").context("discard.target")?)?);
    }
    for e in seq(body, "rename") {
        let from = parse_field(get(&e, "from").context("rename.from")?)?;
        let to = if let Some(t) = get(&e, "to_target") {
            parse_field(t)?
        } else {
            let k = get(&e, "to").and_then(scalar).context("rename.to")?;
            key_field(&from, &k, scope)
        };
        ops.rename.push(Rename { from, to });
    }
    for e in seq(body, "rename_prefix") {
        ops.rename_prefix.push(RenamePrefix {
            from: parse_field(get(&e, "from").context("rename_prefix.from")?)?,
            to: get(&e, "to").and_then(scalar).context("rename_prefix.to")?,
            replace: str_pairs(get(&e, "replace")),
        });
    }
    for e in seq(body, "split") {
        let from = parse_field(get(&e, "from").context("split.from")?)?;
        let to = seq(&e, "to")
            .iter()
            .filter_map(scalar)
            .map(|k| key_field(&from, &k, scope))
            .collect();
        ops.split.push(Split {
            separator: get(&e, "separator").and_then(scalar).unwrap_or_default(),
            from,
            to,
        });
    }
    for e in seq(body, "add") {
        let target = parse_field(get(&e, "target").context("add.target")?)?;
        let from = match get(&e, "from") {
            Some(Value::Sequence(s)) => s.iter().map(parse_field).collect::<Result<_>>()?,
            Some(v) => vec![parse_field(v)?],
            None => vec![],
        };
        ops.add.push(Add {
            target,
            value: get(&e, "value").and_then(scalar),
            from,
            separator: get(&e, "separator").and_then(scalar),
            upsert: get(&e, "upsert").and_then(Value::as_bool).unwrap_or(false),
        });
    }
    for e in seq(body, "map") {
        ops.map.push(Map {
            target: parse_field(get(&e, "target").context("map.target")?)?,
            values: str_pairs(get(&e, "values")),
            unmapped: get(&e, "unmapped")
                .and_then(scalar)
                .unwrap_or_else(|| "keep".into()),
        });
    }
    for e in seq(body, "convert") {
        ops.convert.push((
            parse_field(get(&e, "target").context("convert.target")?)?,
            get(&e, "type").and_then(scalar).unwrap_or_default(),
        ));
    }
    Ok(ops)
}

const KINDS: [(&str, Kind); 5] = [
    ("trace_transform", Kind::Trace),
    ("metric_transform", Kind::Metric),
    ("log_transform", Kind::Log),
    ("entity_transform", Kind::Entity),
    ("metric_filter", Kind::MetricFilter),
];

fn parse_policy(v: &Value) -> Result<Policy> {
    let id = get(v, "id")
        .and_then(scalar)
        .unwrap_or_else(|| "<no id>".into());
    let (kind, body) = KINDS
        .iter()
        .find_map(|(k, kind)| get(v, k).map(|b| (*kind, b)))
        .ok_or_else(|| anyhow!("{id}: no known policy type"))?;
    let matchers = seq(body, "matches")
        .iter()
        .map(parse_matcher)
        .collect::<Result<_>>()
        .with_context(|| id.clone())?;
    let ops = parse_ops(body, kind.scope()).with_context(|| id.clone())?;
    Ok(Policy {
        id,
        kind,
        matchers,
        ops,
    })
}

pub fn parse_file(text: &str) -> Result<TransformFile> {
    let v: Value = serde_yaml::from_str(text)?;
    let url = |side: &str| {
        get(&v, side)
            .and_then(|s| get(s, "schema_url"))
            .and_then(scalar)
            .unwrap_or_default()
    };
    let path = |side: &str| {
        get(&v, side)
            .and_then(|s| get(s, "registry_path"))
            .and_then(scalar)
    };
    let mut f = TransformFile {
        source_url: url("source"),
        target_url: url("target"),
        source_path: path("source"),
        target_path: path("target"),
        gaps: get(&v, "gaps").cloned(),
        ..Default::default()
    };
    for p in seq(&v, "policies") {
        match parse_policy(&p) {
            Ok(p) => f.policies.push(p),
            Err(e) => f.parse_errors.push(format!("{e:#}")),
        }
    }
    Ok(f)
}

/// Canonical form used to compare declared and computed gaps.
pub type Gaps = BTreeMap<String, BTreeMap<String, GapEntry>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GapEntry {
    Missing,
    MissingAttributes(Vec<String>),
}

pub fn parse_gaps(v: &Value) -> Gaps {
    let mut out = Gaps::new();
    let Some(m) = v.as_mapping() else { return out };
    for (sig, sigs) in m {
        let sig = scalar(sig).unwrap_or_default();
        let Some(sigs) = sigs.as_mapping() else {
            continue;
        };
        for (name, entry) in sigs {
            let name = scalar(name).unwrap_or_default();
            let e = if get(entry, "missing").and_then(Value::as_bool) == Some(true) {
                GapEntry::Missing
            } else {
                let mut a: Vec<String> = seq(entry, "missing_attributes")
                    .iter()
                    .filter_map(scalar)
                    .collect();
                a.sort();
                GapEntry::MissingAttributes(a)
            };
            _ = out.entry(sig.clone()).or_default().insert(name, e);
        }
    }
    out
}
