mod checks;
mod engine;
mod model;
mod policy;
mod preprocess;
mod resolve;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::checks::{run_file, FileReport, Registries};

const EXAMPLES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples");
const TOP: usize = 8;

/// Usage: `transform-check [--write-gaps] [examples_dir] [out_dir]`.
fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let registries = std::env::var("TRANSFORM_CHECK_REGISTRIES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("registries"));
    let mut regs = Registries::new(registries, root.join("target/patched-registries"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let write_gaps = args.iter().any(|a| a == "--write-gaps");
    let mut pos = args.iter().filter(|a| !a.starts_with("--"));
    let examples = pos.next().cloned().unwrap_or_else(|| EXAMPLES.to_owned());
    let out_dir = pos
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/report"));

    let mut files: Vec<PathBuf> = std::fs::read_dir(&examples)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .flat_map(|d| {
            std::fs::read_dir(d.path())
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
        })
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .collect();
    files.sort();

    let mut reports = vec![];
    for f in &files {
        let area = f
            .parent()
            .and_then(Path::file_name)
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let name = f
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        eprintln!("checking {area}/{name}");
        let text = std::fs::read_to_string(f)?;
        let r = run_file(
            &mut regs,
            &area,
            &name,
            f.parent().unwrap_or(Path::new(".")),
            &text,
        )?;
        let gaps_dir = out_dir.join("gaps").join(&area);
        std::fs::create_dir_all(&gaps_dir)?;
        let block = match (&r.gaps_yaml, &r.skip_reason) {
            (Some(y), _) => y.clone(),
            (None, Some(why)) => format!("# gaps: not checked, {why}\n"),
            (None, None) => String::new(),
        };
        std::fs::write(gaps_dir.join(name.replace(".yaml", ".gaps.yaml")), &block)?;
        let replacement = match (&r.gaps_yaml, &r.skip_reason) {
            (Some(y), _) => Some(y.clone()),
            (None, Some(why)) => Some(format!("# gaps: not checked, {why}\n")),
            (None, None) => None,
        };
        if let (true, Some(rep)) = (write_gaps, replacement) {
            let updated = replace_gaps_block(&text, &rep);
            if updated != text {
                std::fs::write(f, updated)?;
            }
        }
        reports.push(r);
    }

    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(out_dir.join("REPORT.md"), render(&reports, Some(TOP)))?;
    std::fs::write(out_dir.join("DETAILS.md"), render(&reports, None))?;
    println!("{}", summary_table(&reports));
    eprintln!("report: {}", out_dir.join("REPORT.md").display());
    Ok(())
}

/// Replaces the top-level `gaps:` block (or a previous `# gaps:` comment line) with `block`;
/// inserts it before `policies:` when there is none.
fn replace_gaps_block(text: &str, block: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let start = lines.iter().position(|l| {
        l.starts_with("gaps:")
            || l.starts_with("# gaps:")
            || l.starts_with("# strict check skipped")
    });
    let (start, end) = match start {
        Some(s) => {
            let mut e = s + 1;
            if lines[s].starts_with("gaps:") {
                while e < lines.len() && lines[e].starts_with(' ') {
                    e += 1;
                }
            }
            (s, e)
        }
        None => match lines.iter().position(|l| l.starts_with("policies:")) {
            Some(p) => (p, p),
            None => return text.to_owned(),
        },
    };
    let mut out: String = lines[..start].concat();
    out.push_str(block);
    out.push_str(&lines[end..].concat());
    out
}

fn summary_table(reports: &[FileReport]) -> String {
    let mut s = String::from(
        "| file | regs (src/tgt) | parse err | loose err | not-in-src (interm.) | tgt changed (definite/value-dep) | conflicts | no-src-match | strict: unproducible / attr gaps / unknown attrs | required missing | strict | gaps before -> after |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for r in reports {
        let strict = match &r.strict_pass {
            Some(true) if r.strict_scope_count == 0 => "PASS (vacuous)",
            Some(true) => "PASS",
            Some(false) => "FAIL",
            None => "skipped",
        };
        let after = r
            .gaps
            .as_ref()
            .map(|g| g.values().map(|m| m.len()).sum::<usize>().to_string())
            .unwrap_or_else(|| "skipped".into());
        let before = r
            .declared_gaps
            .map(|n| n.to_string())
            .unwrap_or_else(|| "-".into());
        let _ = writeln!(
            s,
            "| {}/{} | {}/{} | {} | {} | {} | {}/{} | {} | {} | {} | {} | {} | {before} -> {after} |",
            r.area,
            r.file,
            r.source_label.as_deref().unwrap_or("-"),
            r.target_label.as_deref().unwrap_or("-"),
            r.parse_errors.len(),
            r.loose_errors.len(),
            r.intermediate.len(),
            r.target_changed.len(),
            r.target_changed_value.len(),
            r.conflicts.len(),
            r.dead_policies.len(),
            r.strict_counts
                .map(|(a, b)| format!("{a} / {b} / {}", r.unknown_attrs.len()))
                .unwrap_or_else(|| "n/a".into()),
            if r.missing_required.is_empty() {
                "0".to_owned()
            } else {
                format!("{} REJECTED", r.missing_required.len())
            },
            strict,
        );
    }
    s
}

fn section(out: &mut String, title: &str, items: &[String], top: Option<usize>) {
    if items.is_empty() {
        return;
    }
    let _ = writeln!(out, "\n**{title}** ({})\n", items.len());
    let n = top.unwrap_or(items.len());
    for i in items.iter().take(n) {
        let _ = writeln!(out, "- {i}");
    }
    if items.len() > n {
        let _ = writeln!(out, "- ... {} more (see DETAILS.md)", items.len() - n);
    }
}

fn render(reports: &[FileReport], top: Option<usize>) -> String {
    let mut out = String::from(
        "# Schema transformation checker report\n\nHow checks work and known limitations: see `README.md` next to the checker.\n\n## Summary\n\n",
    );
    out.push_str(&summary_table(reports));
    let mut by_area: BTreeMap<&str, Vec<&FileReport>> = BTreeMap::new();
    for r in reports {
        by_area.entry(&r.area).or_default().push(r);
    }
    for (area, rs) in by_area {
        let _ = writeln!(out, "\n## {area}");
        for r in rs {
            let _ = writeln!(out, "\n### {}\n", r.file);
            for n in &r.notes {
                let _ = writeln!(out, "> {n}\n");
            }
            section(&mut out, "Parse errors", &r.parse_errors, top);
            section(
                &mut out,
                "Loose: errors (unknown keys/names, writes not in target, scope mismatch)",
                &r.loose_errors,
                top,
            );
            section(&mut out, "Loose: warnings", &r.loose_warnings, top);
            section(
                &mut out,
                "Loose: not in source, found in an intermediate release",
                &r.intermediate,
                top,
            );
            section(
                &mut out,
                "Loose: info (matcher/read keys only in target, deprecated targets)",
                &r.loose_info,
                top,
            );
            section(
                &mut out,
                "Loose: renamed metric property mismatch (instrument/unit)",
                &r.properties,
                top,
            );
            section(
                &mut out,
                "Target-shaped data changed (definite)",
                &r.target_changed,
                top,
            );
            section(&mut out, "Conflicts", &r.conflicts, top);
            section(
                &mut out,
                "Policies that match no source definition (schema mode)",
                &r.dead_policies,
                top,
            );
            section(
                &mut out,
                "Strict: target signals not producible",
                &r.unproducible,
                top,
            );
            section(
                &mut out,
                "ERROR: produced target signals missing a required attribute (file rejected)",
                &r.missing_required,
                top,
            );
            section(
                &mut out,
                "Strict: missing conditionally required / recommended attributes",
                &r.missing_attrs,
                top,
            );
            section(
                &mut out,
                "Strict: unknown attributes (source signal lists no attributes; not in gaps)",
                &r.unknown_attrs,
                top,
            );
            section(
                &mut out,
                "Value-dependent (not in gaps): target-shaped data may change",
                &r.target_changed_value,
                top,
            );
            section(&mut out, "Value-dependent (not in gaps): target signals produced only when value matchers hold", &r.strict_value_dep, top);
            match &r.skip_reason {
                Some(why) => {
                    let _ = writeln!(out, "\n**Strict:** skipped. `gaps: skipped` ({why}).");
                }
                None => {
                    let _ = writeln!(
                        out,
                        "\n**Strict:** {} ({} in-scope target signals). Declared `gaps`: {}.",
                        if r.strict_pass == Some(true) {
                            "PASS"
                        } else {
                            "FAIL"
                        },
                        r.strict_scope_count,
                        r.declared_gaps_status,
                    );
                }
            }
            for d in &r.declared_gaps_diff {
                let _ = writeln!(out, "- {d}");
            }
        }
    }
    out
}
