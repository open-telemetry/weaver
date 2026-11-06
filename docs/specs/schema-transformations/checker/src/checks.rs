//! The checks: loose references, target-shaped unchanged, conflicts, strict completeness.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::engine::{apply, disjoint, match_on, matching, may_match, sig_of, Out};
use crate::model::{Def, Level, Registry, Sig};
use crate::policy::{parse_file, parse_gaps, Field, GapEntry, Gaps, Kind, Policy, Pred, Scope};
use crate::preprocess::prepare;
use crate::resolve::resolve;

pub struct Registries {
    root: PathBuf,
    work: PathBuf,
    cache: HashMap<String, Option<Registry>>,
    pub errors: HashMap<String, String>,
    pub preprocess_notes: BTreeMap<String, Vec<String>>,
}

fn semver(label: &str) -> Option<(u32, u32, u32)> {
    let v = label
        .trim_start_matches("gen-ai-dev-")
        .trim_end_matches("-dev");
    let mut it = v.split('.').map(|p| p.parse::<u32>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

impl Registries {
    /// `root` holds the registry copies; patched copies go to `work`.
    pub fn new(root: PathBuf, work: PathBuf) -> Self {
        Self {
            root,
            work,
            cache: HashMap::new(),
            errors: HashMap::new(),
            preprocess_notes: BTreeMap::new(),
        }
    }

    fn load(&mut self, label: &str) {
        self.load_at(label, None);
    }

    /// Loads `label` from `explicit` (a `registry_path`) or from the registries dir.
    fn load_at(&mut self, label: &str, explicit: Option<PathBuf>) {
        if self.cache.contains_key(label) {
            return;
        }
        let dir = self.root.join(label);
        let path = explicit.filter(|p| p.exists()).or_else(|| {
            ["model", "semantic_conventions"]
                .iter()
                .map(|d| dir.join(d))
                .find(|p| p.exists())
        });
        let reg = match path {
            None => {
                _ = self.errors.insert(label.into(), "no local registry".into());
                None
            }
            Some(p) => {
                let prepared = prepare(&p, &self.work.join(label));
                match prepared.and_then(|(p, notes)| {
                    if !notes.is_empty() {
                        _ = self.preprocess_notes.insert(label.into(), notes);
                    }
                    resolve(&p, &[])
                }) {
                    Ok(s) => {
                        let reg = Registry::from_schema(label, &s);
                        if let Ok(name) = std::env::var("DUMP_DEF") {
                            for d in reg.defs.iter().filter(|d| d.name == name) {
                                eprintln!(
                                    "[{label}] {} {:?} {:?} {:?}",
                                    d.id, d.span_kind, d.system, d.attrs
                                );
                            }
                        }
                        Some(reg)
                    }
                    Err(e) => {
                        _ = self.errors.insert(label.into(), e.to_string());
                        None
                    }
                }
            }
        };
        _ = self.cache.insert(label.into(), reg);
    }

    /// Plain semconv versions strictly between `a` and `b`.
    fn intermediates(&mut self, a: &str, b: &str) -> Vec<String> {
        let (Some(va), Some(vb)) = (semver(a), semver(b)) else {
            return vec![];
        };
        let (lo, hi) = if va < vb { (va, vb) } else { (vb, va) };
        let mut labels: Vec<String> = std::fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .filter(|l| !l.starts_with("gen-ai") && semver(l).is_some_and(|v| v > lo && v < hi))
            .collect();
        labels.sort_by_key(|l| semver(l));
        labels.iter().for_each(|l| self.load(l));
        labels
    }
}

fn label_of(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://opentelemetry.io/schemas/") {
        return rest.replace('/', "-");
    }
    if let Some(rest) = url.strip_prefix("https://grpc.io/schemas/") {
        return format!("grpc-{rest}");
    }
    url.to_owned()
}

#[derive(Default)]
pub struct FileReport {
    pub area: String,
    pub file: String,
    pub source_label: Option<String>,
    pub target_label: Option<String>,
    pub notes: Vec<String>,
    pub parse_errors: Vec<String>,
    pub loose_errors: Vec<String>,
    pub loose_warnings: Vec<String>,
    pub intermediate: Vec<String>,
    pub loose_info: Vec<String>,
    pub properties: Vec<String>,
    pub target_changed: Vec<String>,
    pub target_changed_value: Vec<String>,
    pub conflicts: Vec<String>,
    pub dead_policies: Vec<String>,
    pub unproducible: Vec<String>,
    pub missing_attrs: Vec<String>,
    /// Produced target signals without a required attribute; rejects the file.
    pub missing_required: Vec<String>,
    pub unknown_attrs: Vec<String>,
    pub strict_value_dep: Vec<String>,
    pub strict_counts: Option<(usize, usize)>,
    pub strict_pass: Option<bool>,
    pub strict_scope_count: usize,
    /// Why strict was skipped (no registry for a side).
    pub skip_reason: Option<String>,
    pub gaps: Option<Gaps>,
    pub gaps_yaml: Option<String>,
    pub declared_gaps: Option<usize>,
    pub declared_gaps_status: String,
    pub declared_gaps_diff: Vec<String>,
}

/// Signal namespaces each example area is scoped to for strict completeness.
fn area_scope(area: &str) -> (Vec<&'static str>, bool) {
    match area {
        "http" => (vec!["http"], false),
        "db" => (vec!["db"], false),
        "rpc" => (vec!["rpc"], false),
        "grpc" => (vec!["rpc", "grpc"], false),
        "gen-ai" => (vec!["gen_ai", "mcp"], false),
        "k8s" => (vec!["k8s"], false),
        "code" => (vec!["code"], true),
        _ => (vec![], false),
    }
}

fn root_ns(d: &Def) -> &str {
    let n = if d.sig == Sig::Span {
        d.name
            .strip_prefix("span.")
            .or_else(|| d.name.strip_prefix("trace."))
            .unwrap_or(&d.name)
    } else {
        &d.name
    };
    n.split('.').next().unwrap_or(n)
}

fn in_scope(d: &Def, area: &str) -> bool {
    if area == "grpc" {
        // Native side: grpc.* metrics and span.grpc.*; OTel side: rpc metrics and grpc spans.
        return match d.sig {
            Sig::Metric => d.name.starts_with("grpc.") || d.name.starts_with("rpc."),
            Sig::Span => {
                d.name.starts_with("span.grpc.")
                    || d.system.as_ref().is_some_and(|(_, v)| v == "grpc")
            }
            _ => false,
        };
    }
    let (ns, by_attr) = area_scope(area);
    if by_attr {
        d.attrs
            .keys()
            .any(|k| ns.iter().any(|n| k.split('.').next() == Some(n)))
    } else {
        ns.contains(&root_ns(d))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Match,
    ReadSrc,
    ReadAny,
    Write,
}

struct Ref {
    policy: String,
    role: Role,
    what: &'static str,
    key: String,
    name: Option<Sig>,
}

fn collect_refs(policies: &[Policy], scope_errors: &mut Vec<String>) -> Vec<Ref> {
    let mut refs = vec![];
    for p in policies {
        let discarded: BTreeSet<&str> = p
            .ops
            .discard
            .iter()
            .filter_map(|f| f.attr_key().map(|(_, k)| k))
            .collect();
        let mut push = |role, f: &Field, what: &'static str, out: &mut Vec<Ref>| match f {
            Field::Attr(s, k) => {
                let ok = *s == p.kind.scope()
                    || *s == Scope::Resource
                    || (p.kind == Kind::Metric && *s == Scope::Datapoint);
                if !ok {
                    scope_errors.push(format!(
                        "{}: {what} uses {s:?} attribute `{k}` in a {:?} policy",
                        p.id, p.kind
                    ));
                }
                let role = if role == Role::Write && discarded.contains(k.as_str()) {
                    Role::ReadAny
                } else {
                    role
                };
                out.push(Ref {
                    policy: p.id.clone(),
                    role,
                    what,
                    key: k.clone(),
                    name: None,
                });
            }
            _ => {}
        };
        for m in &p.matchers {
            match (&m.field, &m.pred) {
                (Field::MetricName, Pred::Equals(v)) if !m.negate => refs.push(Ref {
                    policy: p.id.clone(),
                    role: Role::Match,
                    what: "matcher",
                    key: v.clone(),
                    name: Some(Sig::Metric),
                }),
                (Field::EventName, Pred::Equals(v)) if !m.negate => refs.push(Ref {
                    policy: p.id.clone(),
                    role: Role::Match,
                    what: "matcher",
                    key: v.clone(),
                    name: Some(Sig::Event),
                }),
                (f, _) => push(Role::Match, f, "matcher", &mut refs),
            }
        }
        for f in &p.ops.remove {
            push(Role::ReadSrc, f, "remove", &mut refs);
        }
        for r in &p.ops.rename {
            push(Role::ReadSrc, &r.from, "rename.from", &mut refs);
            push(Role::Write, &r.to, "rename.to", &mut refs);
        }
        for r in &p.ops.rename_prefix {
            push(Role::ReadSrc, &r.from, "rename_prefix.from", &mut refs);
            push(
                Role::Write,
                &Field::Attr(p.kind.scope(), r.to.clone()),
                "rename_prefix.to",
                &mut refs,
            );
        }
        for s in &p.ops.split {
            push(Role::ReadSrc, &s.from, "split.from", &mut refs);
            for t in &s.to {
                push(Role::Write, t, "split.to", &mut refs);
            }
        }
        for a in &p.ops.add {
            for f in &a.from {
                push(Role::ReadAny, f, "add.from", &mut refs);
            }
            match (&a.target, &a.value) {
                (Field::MetricName, Some(v)) => refs.push(Ref {
                    policy: p.id.clone(),
                    role: Role::Write,
                    what: "add metric name",
                    key: v.clone(),
                    name: Some(Sig::Metric),
                }),
                (t, _) => push(Role::Write, t, "add.target", &mut refs),
            }
        }
        for m in &p.ops.map {
            push(Role::ReadAny, &m.target, "map", &mut refs);
        }
        for (f, _) in &p.ops.convert {
            push(Role::ReadAny, f, "convert", &mut refs);
        }
        for f in &p.ops.discard {
            push(Role::ReadAny, f, "discard", &mut refs);
        }
    }
    refs
}

fn has(reg: &Registry, key: &str, name: Option<Sig>) -> bool {
    match name {
        Some(Sig::Metric) => reg.metric_names.contains(key),
        Some(Sig::Event) => reg.event_names.contains(key),
        _ => reg.has_attr(key),
    }
}

fn loose(
    r: &mut FileReport,
    refs: &[Ref],
    src: Option<&Registry>,
    tgt: Option<&Registry>,
    inter: &[&Registry],
) {
    let written: BTreeSet<&str> = refs
        .iter()
        .filter(|x| x.role == Role::Write)
        .map(|x| x.key.as_str())
        .collect();
    let in_inter = |key: &str, name: Option<Sig>| -> Option<&str> {
        inter
            .iter()
            .find(|g| has(g, key, name))
            .map(|g| g.label.as_str())
    };
    let mut seen = BTreeSet::new();
    for x in refs {
        if !seen.insert((x.policy.clone(), x.what, x.key.clone())) {
            continue;
        }
        let kind = match x.name {
            Some(Sig::Metric) => "metric",
            Some(_) => "event",
            None => "key",
        };
        let loc = format!("{}: {} {kind} `{}`", x.policy, x.what, x.key);
        let in_src = src.map(|s| has(s, &x.key, x.name));
        let in_tgt = tgt.map(|t| has(t, &x.key, x.name));
        match x.role {
            Role::Match | Role::ReadSrc => {
                if in_src != Some(false) {
                    continue;
                }
                if let Some(v) = in_inter(&x.key, x.name) {
                    r.intermediate.push(format!("{loc} (first seen in {v})"));
                } else if in_tgt == Some(true) || written.contains(x.key.as_str()) {
                    r.loose_info
                        .push(format!("{loc} not in source, only in target"));
                } else {
                    r.loose_errors
                        .push(format!("{loc} not in source/target/intermediates"));
                }
            }
            Role::ReadAny => {
                if in_src == Some(true) || in_tgt == Some(true) || written.contains(x.key.as_str())
                {
                    continue;
                }
                if src.is_none() && tgt.is_none() {
                    continue;
                }
                if let Some(v) = in_inter(&x.key, x.name) {
                    r.intermediate.push(format!("{loc} (first seen in {v})"));
                } else if in_src.is_some() && in_tgt.is_some() {
                    r.loose_errors
                        .push(format!("{loc} not in source or target"));
                } else {
                    r.loose_info.push(format!(
                        "{loc} not in the available side (other side has no registry)"
                    ));
                }
            }
            Role::Write => match in_tgt {
                Some(false) => {
                    let hint = if in_src == Some(true) {
                        " (exists in source)"
                    } else {
                        ""
                    };
                    if x.what == "rename_prefix.to" {
                        // Template keys (`http.request.header.<key>`) were often markdown-only.
                        r.loose_warnings.push(format!(
                            "{loc} template prefix not defined in target registry{hint}"
                        ));
                    } else {
                        r.loose_errors
                            .push(format!("{loc} written but not in target{hint}"));
                    }
                }
                Some(true)
                    if x.name.is_none() && tgt.is_some_and(|t| t.attr_deprecated(&x.key)) =>
                {
                    r.loose_info.push(format!("{loc} is deprecated in target"));
                }
                _ => {}
            },
        }
    }
}

fn describe_change(d: &Def, out: &Out) -> Option<String> {
    let mut parts = vec![];
    if out.name != d.name {
        parts.push(format!("name -> {}", out.name));
    }
    let removed: Vec<&str> = d
        .attrs
        .keys()
        .filter(|k| !out.attrs.contains_key(*k))
        .map(String::as_str)
        .collect();
    let added: Vec<&str> = out
        .attrs
        .keys()
        .filter(|k| !d.attrs.contains_key(*k))
        .map(String::as_str)
        .collect();
    if !removed.is_empty() {
        parts.push(format!("-[{}]", removed.join(", ")));
    }
    if !added.is_empty() {
        parts.push(format!("+[{}]", added.join(", ")));
    }
    parts.extend(out.value_changes.iter().cloned());
    (!parts.is_empty()).then(|| parts.join("; "))
}

fn target_unchanged(r: &mut FileReport, policies: &[Policy], tgt: &Registry) {
    for d in tgt.defs.iter().filter(|d| !d.deprecated && !d.synthetic) {
        let m = matching(policies, d);
        if m.is_empty() {
            continue;
        }
        let describe = |m: &[&Policy]| -> Option<String> {
            let filters: Vec<&str> = m
                .iter()
                .filter(|p| p.kind == Kind::MetricFilter)
                .map(|p| p.id.as_str())
                .collect();
            let out = apply(m, d, tgt);
            let mut desc = describe_change(d, &out);
            if !filters.is_empty() {
                let f = format!("metric_filter may drop series: {}", filters.join(", "));
                desc = Some(desc.map(|s| format!("{s}; {f}")).unwrap_or(f));
            }
            desc.map(|desc| {
                format!(
                    "{} `{}` [{}]: {desc}",
                    d.sig.plural().trim_end_matches('s'),
                    d.name,
                    out.policies.join(", ")
                )
            })
        };
        let base: Vec<&Policy> = m
            .iter()
            .copied()
            .filter(|p| match_on(p, d) == Some(false))
            .collect();
        if let Some(s) = describe(&base) {
            r.target_changed.push(s);
        } else if let Some(s) = describe(&m) {
            r.target_changed_value.push(s);
        }
    }
}

fn op_conflicts(p: &Policy, q: &Policy, d: &Def) -> Vec<String> {
    let mut c = vec![];
    let on_def = |f: &Field| f.attr_key().is_some_and(|(_, k)| d.attrs.contains_key(k));
    for a in &p.ops.rename {
        for b in &q.ops.rename {
            if a.from == b.from && a.to != b.to {
                c.push(format!("rename {} -> {} vs {}", a.from, a.to, b.to));
            }
            if a.from != b.from && a.to == b.to && on_def(&a.from) && on_def(&b.from) {
                c.push(format!("rename {} and {} both -> {}", a.from, b.from, a.to));
            }
        }
    }
    for a in &p.ops.rename_prefix {
        for b in &q.ops.rename_prefix {
            if a.from == b.from && a != b {
                c.push(format!("rename_prefix {} -> {} vs {}", a.from, a.to, b.to));
            }
        }
    }
    for a in &p.ops.split {
        for b in &q.ops.split {
            if a.from == b.from && a != b {
                c.push(format!("split {}", a.from));
            }
        }
    }
    for a in &p.ops.add {
        for b in &q.ops.add {
            if a.target == b.target && a != b {
                let show = |x: &crate::policy::Add| {
                    x.value.clone().unwrap_or_else(|| {
                        format!(
                            "from[{}]",
                            x.from
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    })
                };
                c.push(format!("add {} = {} vs {}", a.target, show(a), show(b)));
            }
        }
    }
    for a in &p.ops.map {
        for b in &q.ops.map {
            if a.target == b.target && a != b {
                c.push(format!("map {}", a.target));
            }
        }
    }
    for (fa, ta) in &p.ops.convert {
        for (fb, tb) in &q.ops.convert {
            if fa == fb && ta != tb {
                c.push(format!("convert {fa} {ta} vs {tb}"));
            }
        }
    }
    c
}

fn conflicts(r: &mut FileReport, policies: &[Policy], defs: &[&Def]) {
    for (i, p) in policies.iter().enumerate() {
        for q in &policies[i + 1..] {
            if sig_of(p.kind) != sig_of(q.kind) || disjoint(p, q) {
                continue;
            }
            let hit = defs
                .iter()
                .filter(|d| may_match(p, d) && may_match(q, d))
                .find_map(|d| {
                    let ops = op_conflicts(p, q, d);
                    (!ops.is_empty()).then_some((d, ops))
                });
            if let Some((d, ops)) = hit {
                r.conflicts.push(format!(
                    "{} vs {} on `{}`: {}",
                    p.id,
                    q.id,
                    d.name,
                    ops.join("; ")
                ));
            }
        }
    }
}

struct Produced<'a> {
    src: &'a Def,
    value_dep: bool,
    out: Out,
}

fn kind_compatible(a: &Def, b: &Def) -> bool {
    match (&a.span_kind, &b.span_kind) {
        (Some(x), Some(y)) => x == y,
        _ => true,
    }
}

/// Span group id without the `span`/`trace` prefix and span-kind segments.
fn span_stem(id: &str) -> String {
    const SKIP: [&str; 7] = [
        "span", "trace", "client", "server", "internal", "producer", "consumer",
    ];
    id.split('.')
        .filter(|s| !SKIP.contains(s))
        .collect::<Vec<_>>()
        .join(".")
}

/// Policy sets a definition can see: value-independent policies alone, and with each
/// value-dependent policy plus the ones not disjoint from it. The flag marks sets that
/// need a value-dependent policy.
fn variants<'a>(m: &[&'a Policy], d: &Def) -> Vec<(bool, Vec<&'a Policy>)> {
    let (dep, base): (Vec<&Policy>, Vec<&Policy>) =
        m.iter().partition(|p| match_on(p, d) == Some(true));
    let mut out = vec![(false, base.clone())];
    let mut seen = BTreeSet::new();
    for p in &dep {
        let mut v = base.clone();
        v.extend(dep.iter().filter(|q| q.id == p.id || !disjoint(p, q)));
        let key: Vec<String> = v.iter().map(|x| x.id.clone()).collect();
        if seen.insert(key) {
            out.push((true, v));
        }
    }
    out
}

const SYSTEM_KEYS: [&str; 4] = [
    "db.system.name",
    "db.system",
    "rpc.system.name",
    "rpc.system",
];

/// Source spans that can become `t`: same kind and system value after the policies; then
/// system-generic spans carrying the same system key; then the same id stem.
fn span_candidates<'p, 'a>(produced: &'p [Produced<'a>], t: &Def) -> Vec<&'p Produced<'a>> {
    let spans = || {
        produced
            .iter()
            .filter(|p| p.src.sig == Sig::Span && kind_compatible(p.src, t))
    };
    if let Some((_, tv)) = &t.system {
        let exact: Vec<_> = spans()
            .filter(|p| {
                p.out
                    .system
                    .as_ref()
                    .is_some_and(|(_, v)| v.eq_ignore_ascii_case(tv))
            })
            .collect();
        if !exact.is_empty() {
            return exact;
        }
    }
    let stem: Vec<_> = spans()
        .filter(|p| span_stem(&p.src.name) == span_stem(&t.name))
        .collect();
    if !stem.is_empty() {
        return stem;
    }
    // A target span with no attributes at all (native gRPC A72) can only be paired by kind.
    if !t.attrs_listed {
        return spans().collect();
    }
    let Some(key) = SYSTEM_KEYS.iter().find(|k| t.attrs.contains_key(**k)) else {
        return vec![];
    };
    spans()
        .filter(|p| p.out.system.is_none() && p.out.attrs.contains_key(*key))
        .collect()
}

fn strict(
    r: &mut FileReport,
    policies: &[Policy],
    src: &Registry,
    tgt: &Registry,
    area: &str,
) -> Gaps {
    let produced: Vec<Produced<'_>> = src
        .defs
        .iter()
        .filter(|d| d.sig != Sig::Entity)
        .flat_map(|d| {
            let m = matching(policies, d);
            variants(&m, d)
                .into_iter()
                .map(move |(value_dep, v)| Produced {
                    src: d,
                    value_dep,
                    out: apply(&v, d, src),
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let mut gaps = Gaps::new();
    let mut n_missing = 0;
    let mut n_attr = 0;
    for t in tgt
        .defs
        .iter()
        .filter(|d| d.sig != Sig::Entity && !d.deprecated && !d.synthetic && in_scope(d, area))
    {
        r.strict_scope_count += 1;
        let wanted: Vec<&str> = t
            .attrs
            .iter()
            .filter(|(k, l)| {
                l.counts_for_strict()
                    && !tgt.attr_deprecated(k)
                    && !tgt.attrs.get(k.as_str()).is_some_and(|a| a.template)
            })
            .map(|(k, _)| k.as_str())
            .collect();
        let missing_of = |p: &Produced<'_>| -> Vec<&str> {
            wanted
                .iter()
                .filter(|k| !p.out.attrs.contains_key(**k))
                .copied()
                .collect()
        };
        let mut cands: Vec<&Produced<'_>> = if t.sig == Sig::Span {
            span_candidates(&produced, t)
        } else {
            produced
                .iter()
                .filter(|p| p.src.sig == t.sig && p.out.name == t.name)
                .collect()
        };
        cands.sort_by_key(|p| (missing_of(p).len(), p.value_dep));
        if std::env::var("DUMP_PAIR").is_ok_and(|n| n == t.name) {
            for c in &cands {
                eprintln!(
                    "[pair {}] {} dep={} sys={:?} policies={:?} attrs={:?}",
                    t.name,
                    c.src.name,
                    c.value_dep,
                    c.out.system,
                    c.out.policies,
                    c.out.attrs.keys()
                );
            }
        }
        let stab = if t.stable { "stable" } else { "dev" };
        let sig = t.sig.plural().trim_end_matches('s');
        let sig_key = t.sig.plural().to_owned();
        let Some(best) = cands.first() else {
            n_missing += 1;
            r.unproducible.push(format!("{sig} `{}` ({stab})", t.name));
            _ = gaps
                .entry(sig_key)
                .or_default()
                .insert(t.name.clone(), GapEntry::Missing);
            continue;
        };
        if cands.iter().all(|p| p.value_dep) {
            r.strict_value_dep.push(format!(
                "{sig} `{}` produced from `{}` only when value matchers hold [{}]",
                t.name,
                best.src.name,
                best.out.policies.join(", ")
            ));
        }
        if t.sig == Sig::Metric {
            let s = best.src;
            if s.instrument != t.instrument || s.unit != t.unit {
                r.properties.push(format!(
                    "`{}` -> `{}`: {:?}/{:?} vs {:?}/{:?}",
                    s.name, t.name, s.instrument, s.unit, t.instrument, t.unit
                ));
            }
        }
        let miss = missing_of(best);
        if miss.is_empty() {
            continue;
        }
        let via = if best.src.name != t.name {
            format!(" (from `{}`)", best.src.name)
        } else {
            String::new()
        };
        if !best.src.attrs_listed {
            r.unknown_attrs.push(format!(
                "{sig} `{}` ({stab}){via}: {}",
                t.name,
                miss.join(", ")
            ));
            continue;
        }
        let (req, rest): (Vec<&str>, Vec<&str>) = miss
            .iter()
            .partition(|k| t.attrs.get(**k) == Some(&Level::Required));
        if !req.is_empty() {
            r.missing_required.push(format!(
                "{sig} `{}` ({stab}){via}: {}",
                t.name,
                req.join(", ")
            ));
        }
        if rest.is_empty() {
            continue;
        }
        n_attr += 1;
        r.missing_attrs.push(format!(
            "{sig} `{}` ({stab}){via}: {}",
            t.name,
            rest.join(", ")
        ));
        let mut rest: Vec<String> = rest.iter().map(|s| s.to_string()).collect();
        rest.sort();
        _ = gaps
            .entry(sig_key)
            .or_default()
            .insert(t.name.clone(), GapEntry::MissingAttributes(rest));
    }
    r.strict_counts = Some((n_missing, n_attr));
    gaps
}

fn gaps_yaml(g: &Gaps) -> String {
    let mut s = String::from("gaps:");
    if g.is_empty() {
        s.push_str(" {}\n");
        return s;
    }
    s.push('\n');
    for (sig, entries) in g {
        let _ = writeln!(s, "  {sig}:");
        for (name, e) in entries {
            let _ = writeln!(s, "    {name}:");
            match e {
                GapEntry::Missing => s.push_str("      missing: true\n"),
                GapEntry::MissingAttributes(a) => {
                    let _ = writeln!(s, "      missing_attributes: [{}]", a.join(", "));
                }
            }
        }
    }
    s
}

fn compare_gaps(declared: &Gaps, computed: &Gaps) -> Vec<String> {
    let mut diff = vec![];
    let flat = |g: &Gaps| -> BTreeMap<(String, String), GapEntry> {
        g.iter()
            .flat_map(|(s, m)| {
                m.iter()
                    .map(move |(n, e)| ((s.clone(), n.clone()), e.clone()))
            })
            .collect()
    };
    let (d, c) = (flat(declared), flat(computed));
    for (k, e) in &c {
        match d.get(k) {
            None => diff.push(format!("undeclared gap {}/{}: {e:?}", k.0, k.1)),
            Some(de) if de != e => {
                diff.push(format!("{}/{}: declared {de:?}, computed {e:?}", k.0, k.1))
            }
            _ => {}
        }
    }
    for k in d.keys().filter(|k| !c.contains_key(*k)) {
        diff.push(format!(
            "declared gap {}/{} not found by strict check",
            k.0, k.1
        ));
    }
    diff
}

pub fn run_file(
    regs: &mut Registries,
    area: &str,
    file: &str,
    dir: &Path,
    text: &str,
) -> Result<FileReport> {
    let tf = parse_file(text)?;
    let mut r = FileReport {
        area: area.into(),
        file: file.into(),
        parse_errors: tf.parse_errors.clone(),
        ..Default::default()
    };
    let (sl, tl) = (label_of(&tf.source_url), label_of(&tf.target_url));
    let inter_labels = regs.intermediates(&sl, &tl);
    regs.load_at(&sl, tf.source_path.as_ref().map(|p| dir.join(p)));
    regs.load_at(&tl, tf.target_path.as_ref().map(|p| dir.join(p)));
    for l in [&sl, &tl] {
        if let Some(e) = regs.errors.get(l.as_str()) {
            r.notes.push(format!(
                "registry `{l}`: {e}; checks needing it are skipped"
            ));
        }
        if let Some(n) = regs.preprocess_notes.get(l.as_str()) {
            r.notes.push(format!(
                "registry `{l}`: constraints rewritten: {}",
                n.join("; ")
            ));
        }
    }
    if area == "k8s" {
        r.notes
            .push("`1.18.0` here is the K8s Collector receiver schema, not semconv 1.18.0.".into());
    }
    let src = regs.cache.get(&sl).and_then(Option::as_ref);
    let tgt = regs.cache.get(&tl).and_then(Option::as_ref);
    r.source_label = src.map(|_| sl.clone());
    r.target_label = tgt.map(|_| tl.clone());
    let inter: Vec<&Registry> = inter_labels
        .iter()
        .filter_map(|l| regs.cache.get(l).and_then(Option::as_ref))
        .collect();

    let mut scope_errors = vec![];
    let refs = collect_refs(&tf.policies, &mut scope_errors);
    r.loose_errors.extend(scope_errors);
    loose(&mut r, &refs, src, tgt, &inter);

    if let Some(s) = src {
        for p in &tf.policies {
            if !s.defs.iter().any(|d| may_match(p, d)) {
                r.dead_policies.push(p.id.clone());
            }
        }
    }
    if let Some(t) = tgt {
        target_unchanged(&mut r, &tf.policies, t);
    }
    let defs: Vec<&Def> = src
        .into_iter()
        .chain(tgt)
        .flat_map(|g| g.defs.iter())
        .collect();
    conflicts(&mut r, &tf.policies, &defs);

    let declared = tf.gaps.as_ref().map(parse_gaps);
    r.declared_gaps = declared.as_ref().map(|g| g.values().map(|m| m.len()).sum());
    if let (Some(s), Some(t)) = (src, tgt) {
        let gaps = strict(&mut r, &tf.policies, s, t, area);
        r.strict_pass = Some(gaps.is_empty());
        r.gaps_yaml = Some(gaps_yaml(&gaps));
        match &declared {
            None => r.declared_gaps_status = "absent".into(),
            Some(d) => {
                r.declared_gaps_diff = compare_gaps(d, &gaps);
                r.declared_gaps_status = if r.declared_gaps_diff.is_empty() {
                    "match".into()
                } else {
                    format!("{} diffs", r.declared_gaps_diff.len())
                };
            }
        }
        r.gaps = Some(gaps);
    } else {
        let side = if src.is_none() {
            format!("source ({sl})")
        } else {
            format!("target ({tl})")
        };
        r.skip_reason = Some(format!("no registry for {side}"));
        r.declared_gaps_status = format!("skipped: no registry for {side}");
    }
    Ok(r)
}
