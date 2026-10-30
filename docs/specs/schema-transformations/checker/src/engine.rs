//! Schema-mode policy application: "may match" on definitions, attribute-key effects.

use std::collections::BTreeMap;

use regex::Regex;
use weaver_semconv::v1::attribute::{AttributeType, PrimitiveOrArrayTypeSpec, ValueSpec};

use crate::model::{Def, Level, Registry, Sig};
use crate::policy::{Field, Kind, Matcher, Policy, Pred, Scope};

pub fn sig_of(kind: Kind) -> Sig {
    match kind {
        Kind::Trace => Sig::Span,
        Kind::Metric | Kind::MetricFilter => Sig::Metric,
        Kind::Log => Sig::Event,
        Kind::Entity => Sig::Entity,
    }
}

fn regex_match(re: &str, v: &str) -> bool {
    Regex::new(re).map(|r| r.is_match(v)).unwrap_or(false)
}

fn pred_on_value(pred: &Pred, v: &str) -> bool {
    match pred {
        Pred::Exists => true,
        Pred::Equals(e) => e.eq_ignore_ascii_case(v),
        Pred::Regex(r) => regex_match(r, v),
        Pred::Cmp(..) => true,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hit {
    No,
    Yes,
    /// Holds only for some attribute / field values.
    Value,
}

fn exact(b: bool) -> Hit {
    if b {
        Hit::Yes
    } else {
        Hit::No
    }
}

fn matcher_hit(kind: Kind, m: &Matcher, d: &Def) -> Hit {
    match &m.field {
        Field::Attr(scope, key) => {
            if *scope == Scope::Resource && kind.scope() != Scope::Resource {
                return if m.pred == Pred::Exists {
                    Hit::Yes
                } else {
                    Hit::Value
                };
            }
            if *scope != kind.scope() {
                return Hit::No;
            }
            if let Some((k, v)) = &d.system {
                if k == key {
                    return exact(pred_on_value(&m.pred, v) != m.negate);
                }
            }
            let level = d.attrs.get(key.as_str());
            match (&m.pred, m.negate) {
                (Pred::Exists, false) => exact(level.is_some()),
                (Pred::Exists, true) => exact(level != Some(&Level::Required)),
                (_, false) if level.is_none() => Hit::No,
                _ => Hit::Value,
            }
        }
        Field::MetricName if d.sig == Sig::Metric => {
            exact(pred_on_value(&m.pred, &d.name) != m.negate)
        }
        Field::EventName if d.sig == Sig::Event => {
            exact(pred_on_value(&m.pred, &d.name) != m.negate)
        }
        Field::SpanKind if d.sig == Sig::Span => match &d.span_kind {
            Some(k) => exact(pred_on_value(&m.pred, k) != m.negate),
            None => Hit::Yes,
        },
        _ if m.pred == Pred::Exists => Hit::Yes,
        _ => Hit::Value,
    }
}

/// `Some(value_dependent)` when the policy may match the definition.
pub fn match_on(p: &Policy, d: &Def) -> Option<bool> {
    if sig_of(p.kind) != d.sig {
        return None;
    }
    let mut dep = false;
    for m in &p.matchers {
        match matcher_hit(p.kind, m, d) {
            Hit::No => return None,
            Hit::Value => dep = true,
            Hit::Yes => {}
        }
    }
    Some(dep)
}

pub fn may_match(p: &Policy, d: &Def) -> bool {
    match_on(p, d).is_some()
}

/// Two matchers on the same field that can't both hold.
fn disjoint_matchers(a: &Matcher, b: &Matcher) -> bool {
    if a.field != b.field {
        return false;
    }
    use Pred::*;
    match ((&a.pred, a.negate), (&b.pred, b.negate)) {
        ((Exists, false), (Exists, true)) | ((Exists, true), (Exists, false)) => true,
        ((Exists, true), (_, false)) | ((_, false), (Exists, true)) => true,
        ((Equals(x), false), (Equals(y), false)) => !x.eq_ignore_ascii_case(y),
        ((Equals(x), false), (Equals(y), true)) | ((Equals(y), true), (Equals(x), false)) => {
            x.eq_ignore_ascii_case(y)
        }
        ((Regex(x), false), (Regex(y), true)) | ((Regex(y), true), (Regex(x), false)) => x == y,
        ((Equals(v), false), (Regex(r), n)) | ((Regex(r), n), (Equals(v), false)) => {
            regex_match(r, v) == n
        }
        ((Equals(v), false), (Cmp(op, n), false)) | ((Cmp(op, n), false), (Equals(v), false)) => {
            match v.parse::<f64>() {
                Ok(x) => !match op.as_str() {
                    "gt" => x > *n,
                    "gte" => x >= *n,
                    "lt" => x < *n,
                    _ => x <= *n,
                },
                Err(_) => false,
            }
        }
        _ => false,
    }
}

pub fn disjoint(p: &Policy, q: &Policy) -> bool {
    p.matchers
        .iter()
        .any(|a| q.matchers.iter().any(|b| disjoint_matchers(a, b)))
}

/// Definition after applying the matching policies.
#[derive(Debug, Clone)]
pub struct Out {
    pub name: String,
    pub attrs: BTreeMap<String, Level>,
    pub value_changes: Vec<String>,
    pub policies: Vec<String>,
    /// System attribute key and value after the policies.
    pub system: Option<(String, String)>,
}

fn type_name(ty: &AttributeType) -> &'static str {
    match ty {
        AttributeType::PrimitiveOrArray(p) => match p {
            PrimitiveOrArrayTypeSpec::Int => "int",
            PrimitiveOrArrayTypeSpec::Double => "double",
            PrimitiveOrArrayTypeSpec::String => "string",
            PrimitiveOrArrayTypeSpec::Boolean => "boolean",
            _ => "other",
        },
        AttributeType::Enum { members } => match members.first().map(|m| &m.value) {
            Some(ValueSpec::Int(_)) => "int",
            Some(ValueSpec::Double(_)) => "double",
            Some(ValueSpec::Bool(_)) => "boolean",
            _ => "string",
        },
        AttributeType::Template(_) => "template",
    }
}

fn enum_values(ty: &AttributeType) -> Option<Vec<String>> {
    match ty {
        AttributeType::Enum { members } => {
            Some(members.iter().map(|m| m.value.to_string()).collect())
        }
        _ => None,
    }
}

fn in_scope(kind: Kind, f: &Field) -> Option<String> {
    match f {
        Field::Attr(s, k) if *s == kind.scope() => Some(k.clone()),
        _ => None,
    }
}

/// Inserted keys from non-tracked fields (resource attributes, datapoint value) count as present.
fn written_key(kind: Kind, f: &Field) -> Option<String> {
    match f {
        Field::Attr(s, k) if *s == kind.scope() || *s == Scope::Datapoint => Some(k.clone()),
        _ => None,
    }
}

pub fn apply(policies: &[&Policy], d: &Def, reg: &Registry) -> Out {
    let mut out = Out {
        name: d.name.clone(),
        attrs: d.attrs.clone(),
        value_changes: vec![],
        policies: policies.iter().map(|p| p.id.clone()).collect(),
        system: d.system.clone(),
    };
    let present = |out: &Out, kind: Kind, f: &Field| -> bool {
        match f {
            Field::Attr(s, k) if *s == kind.scope() => out.attrs.contains_key(k.as_str()),
            _ => true,
        }
    };

    for p in policies {
        for f in &p.ops.remove {
            if let Some(k) = in_scope(p.kind, f) {
                drop_system(&mut out, &k);
                _ = out.attrs.remove(&k);
            }
        }
    }
    for p in policies {
        for r in &p.ops.rename {
            if !present(&out, p.kind, &r.from) {
                continue;
            }
            let lvl = in_scope(p.kind, &r.from)
                .and_then(|k| out.attrs.remove(&k))
                .unwrap_or(Level::Recommended);
            if let Some(k) = written_key(p.kind, &r.to) {
                if let Some((sk, _)) = out.system.as_mut() {
                    if in_scope(p.kind, &r.from).as_deref() == Some(sk.as_str()) {
                        *sk = k.clone();
                    }
                }
                _ = out.attrs.entry(k).or_insert(lvl);
            }
        }
        for rp in &p.ops.rename_prefix {
            let Some(prefix) = in_scope(p.kind, &rp.from) else {
                continue;
            };
            let hits: Vec<String> = out
                .attrs
                .keys()
                .filter(|k| format!("{k}.").starts_with(&prefix))
                .cloned()
                .collect();
            for k in hits {
                let suffix = format!("{k}.")[prefix.len()..].to_owned();
                let new = format!("{}{suffix}", rp.to)
                    .trim_end_matches('.')
                    .to_owned();
                if new == k {
                    if !rp.replace.is_empty() {
                        out.value_changes.push(format!(
                            "{}: rename_prefix replace rewrites {k}.<key>",
                            p.id
                        ));
                    }
                    continue;
                }
                if let Some(l) = out.attrs.remove(&k) {
                    _ = out.attrs.entry(new).or_insert(l);
                }
            }
        }
    }
    for p in policies {
        for s in &p.ops.split {
            if !present(&out, p.kind, &s.from) {
                continue;
            }
            for t in &s.to {
                if let Some(k) = written_key(p.kind, t) {
                    _ = out.attrs.entry(k).or_insert(Level::Recommended);
                }
            }
        }
    }
    for p in policies {
        for a in &p.ops.add {
            if !a.from.is_empty() && !a.from.iter().any(|f| present(&out, p.kind, f)) {
                continue;
            }
            match &a.target {
                Field::MetricName if d.sig == Sig::Metric => {
                    if let Some(v) = &a.value {
                        out.name = v.clone();
                    }
                }
                t => {
                    if let Some(k) = written_key(p.kind, t) {
                        match (out.system.as_mut(), &a.value) {
                            (Some((sk, sv)), Some(v)) => {
                                if *sk == k && (a.upsert || !out.attrs.contains_key(&k)) {
                                    *sv = v.clone();
                                }
                            }
                            (None, Some(v)) if SYSTEM_KEYS.contains(&k.as_str()) => {
                                out.system = Some((k.clone(), v.clone()));
                            }
                            _ => {}
                        }
                        if out.attrs.contains_key(&k) {
                            if a.upsert {
                                out.value_changes
                                    .push(format!("{}: add upsert overwrites {k}", p.id));
                            }
                        } else {
                            _ = out.attrs.insert(k, Level::Recommended);
                        }
                    }
                }
            }
        }
    }
    for p in policies {
        for m in &p.ops.map {
            let Some(k) = in_scope(p.kind, &m.target) else {
                continue;
            };
            if let Some((sk, sv)) = out.system.as_mut() {
                if *sk == k {
                    if let Some((_, to)) = m.values.iter().find(|(f, _)| f == sv) {
                        *sv = to.clone();
                    }
                }
            }
            if !d.attrs.contains_key(&k) || !out.attrs.contains_key(&k) {
                continue;
            }
            let info = reg.attrs.get(&k);
            match info.and_then(|i| enum_values(&i.ty)) {
                Some(members) => {
                    let hit: Vec<&str> = m
                        .values
                        .iter()
                        .filter(|(f, t)| f != t && members.contains(f))
                        .map(|(f, _)| f.as_str())
                        .collect();
                    if !hit.is_empty() {
                        out.value_changes.push(format!(
                            "{}: map rewrites {k} enum values {}",
                            p.id,
                            hit.join(",")
                        ));
                    }
                }
                None => out
                    .value_changes
                    .push(format!("{}: map may rewrite {k} (not an enum)", p.id)),
            }
            if m.unmapped == "remove" {
                out.value_changes
                    .push(format!("{}: map unmapped:remove may drop {k}", p.id));
            }
        }
        for (f, ty) in &p.ops.convert {
            let Some(k) = in_scope(p.kind, f) else {
                continue;
            };
            if !d.attrs.contains_key(&k) || !out.attrs.contains_key(&k) {
                continue;
            }
            if let Some(info) = reg.attrs.get(&k) {
                let have = type_name(&info.ty);
                if have != ty {
                    out.value_changes
                        .push(format!("{}: convert {k} {have} -> {ty}", p.id));
                }
            }
        }
    }
    for p in policies {
        for f in &p.ops.discard {
            if let Some(k) = in_scope(p.kind, f) {
                drop_system(&mut out, &k);
                _ = out.attrs.remove(&k);
            }
        }
    }
    out
}

pub fn matching<'a>(policies: &'a [Policy], d: &Def) -> Vec<&'a Policy> {
    policies.iter().filter(|p| may_match(p, d)).collect()
}

/// System attribute keys whose added value identifies the system.
const SYSTEM_KEYS: [&str; 4] = [
    "db.system.name",
    "db.system",
    "rpc.system.name",
    "rpc.system",
];

fn drop_system(out: &mut Out, key: &str) {
    if out.system.as_ref().is_some_and(|(k, _)| k == key) {
        out.system = None;
    }
}
