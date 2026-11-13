//! Rewrites legacy `constraints:` (rejected by the resolver) into a patched registry copy.
//!
//! `include: <group>` becomes `ref`s to the included group's attributes (following `extends`);
//! `any_of` is dropped.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_yaml::{Mapping, Value};

fn yaml_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            yaml_files(&p, out)?;
        } else if p.extension().is_some_and(|x| x == "yaml" || x == "yml") {
            out.push(p);
        }
    }
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let p = e?.path();
        let t = dst.join(p.file_name().unwrap_or_default());
        if p.is_dir() {
            copy_tree(&p, &t)?;
        } else {
            _ = fs::copy(&p, &t)?;
        }
    }
    Ok(())
}

fn s<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}

struct GroupAttrs {
    extends: Option<String>,
    attrs: Vec<(String, Option<Value>)>,
}

fn index_group(g: &Value) -> Option<(String, GroupAttrs)> {
    let id = s(g, "id")?.to_owned();
    let prefix = s(g, "prefix").unwrap_or_default();
    let attrs = g
        .get("attributes")
        .and_then(Value::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(|a| {
                    let key = match (s(a, "ref"), s(a, "id")) {
                        (Some(r), _) => r.to_owned(),
                        (None, Some(i)) if !prefix.is_empty() => format!("{prefix}.{i}"),
                        (None, Some(i)) => i.to_owned(),
                        _ => return None,
                    };
                    Some((key, a.get("requirement_level").cloned()))
                })
                .collect()
        })
        .unwrap_or_default();
    Some((
        id,
        GroupAttrs {
            extends: s(g, "extends").map(str::to_owned),
            attrs,
        },
    ))
}

fn collect(
    idx: &HashMap<String, GroupAttrs>,
    id: &str,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, Option<Value>)>,
) {
    if !seen.insert(id.to_owned()) {
        return;
    }
    if let Some(g) = idx.get(id) {
        out.extend(g.attrs.iter().cloned());
        if let Some(e) = &g.extends {
            collect(idx, e, seen, out);
        }
    }
}

/// Path to resolve for `src`: itself, or a patched copy under `work` when it uses `constraints`.
pub fn prepare(src: &Path, work: &Path) -> Result<(PathBuf, Vec<String>)> {
    let mut files = vec![];
    yaml_files(src, &mut files)?;
    let docs: Vec<(PathBuf, String)> = files
        .into_iter()
        .map(|f| Ok((fs::read_to_string(&f)?, f)))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .map(|(t, f)| (f, t))
        .collect();
    let uses = |t: &str| {
        t.lines()
            .any(|l| l.trim_start().starts_with("constraints:"))
    };
    if !docs.iter().any(|(_, t)| uses(t)) {
        return Ok((src.to_owned(), vec![]));
    }

    let mut idx = HashMap::new();
    let mut parsed = HashMap::new();
    for (f, t) in &docs {
        let Ok(v) = serde_yaml::from_str::<Value>(t) else {
            continue;
        };
        if let Some(groups) = v.get("groups").and_then(Value::as_sequence) {
            idx.extend(groups.iter().filter_map(index_group));
        }
        if uses(t) {
            _ = parsed.insert(f.clone(), v);
        }
    }

    _ = fs::remove_dir_all(work);
    copy_tree(src, work)?;
    let mut notes = vec![];
    for (f, mut v) in parsed {
        let Some(groups) = v.get_mut("groups").and_then(Value::as_sequence_mut) else {
            continue;
        };
        for g in groups.iter_mut() {
            let Some(m) = g.as_mapping_mut() else {
                continue;
            };
            let Some(cons) = m.remove("constraints") else {
                continue;
            };
            let gid = m
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let includes: Vec<String> = cons
                .as_sequence()
                .into_iter()
                .flatten()
                .filter_map(|c| s(c, "include").map(str::to_owned))
                .collect();
            if includes.is_empty() {
                continue;
            }
            let attrs = m
                .entry(Value::from("attributes"))
                .or_insert_with(|| Value::Sequence(vec![]));
            let Some(seq) = attrs.as_sequence_mut() else {
                continue;
            };
            let mut have: BTreeSet<String> = seq
                .iter()
                .filter_map(|a| s(a, "ref").or_else(|| s(a, "id")).map(str::to_owned))
                .collect();
            let mut added = 0;
            for inc in &includes {
                let mut found = vec![];
                collect(&idx, inc, &mut BTreeSet::new(), &mut found);
                for (key, lvl) in found {
                    if !have.insert(key.clone()) {
                        continue;
                    }
                    let mut a = Mapping::new();
                    _ = a.insert("ref".into(), key.into());
                    // Included groups apply to one system each, so Required becomes conditional.
                    let lvl = match lvl {
                        Some(Value::String(l)) if l == "required" => {
                            let mut c = Mapping::new();
                            _ = c.insert(
                                "conditionally_required".into(),
                                format!("when {inc} applies").into(),
                            );
                            Some(Value::Mapping(c))
                        }
                        other => other,
                    };
                    if let Some(l) = lvl {
                        _ = a.insert("requirement_level".into(), l);
                    }
                    seq.push(Value::Mapping(a));
                    added += 1;
                }
            }
            notes.push(format!(
                "{gid}: inlined {added} attributes from include {}",
                includes.join(", ")
            ));
        }
        let rel = f.strip_prefix(src)?;
        fs::write(work.join(rel), serde_yaml::to_string(&v)?)?;
    }
    Ok((work.to_owned(), notes))
}
