#!/usr/bin/env python3
"""Parse a weaver live-check JSON report, write GitHub Actions step
summary + outputs, and exit with a status code based on the `fail-on`
threshold.

Usage:
    parse-report.py <report-path> <fail-on>

Where <fail-on> is one of: violation | improvement | information | none.
"""
from __future__ import annotations

import json
import os
import sys

LEVEL_ORDER = {"violation": 3, "improvement": 2, "information": 1, "none": 0}


def _findings_by_level(report: dict) -> dict[str, dict[str, int]]:
    """Walk the report and return {level: {message: count}}.

    The flat `statistics.advice_message_counts` field does not carry
    severity, so to render dedicated per-severity sections we need to
    walk the report and group every advisory by its `advice_level`.
    """
    by_level: dict[str, dict[str, int]] = {}

    def visit(node: object) -> None:
        if isinstance(node, dict):
            lcr = node.get("live_check_result")
            if isinstance(lcr, dict):
                for advice in lcr.get("all_advice") or []:
                    if not isinstance(advice, dict):
                        continue
                    lvl = (advice.get("level") or advice.get("advice_level") or "").lower()
                    if lvl not in LEVEL_ORDER or lvl == "none":
                        continue
                    msg = advice.get("message") or ""
                    by_level.setdefault(lvl, {})
                    by_level[lvl][msg] = by_level[lvl].get(msg, 0) + 1
            for v in node.values():
                visit(v)
        elif isinstance(node, list):
            for item in node:
                visit(item)

    visit(report)
    return by_level


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(f"::error::usage: {argv[0]} <report-path> <fail-on>", file=sys.stderr)
        return 2

    report_path, fail_on = argv[1], argv[2].strip().lower()
    if fail_on not in LEVEL_ORDER:
        print(
            f"::error::invalid fail-on '{fail_on}' "
            f"(expected one of {sorted(LEVEL_ORDER)})",
            file=sys.stderr,
        )
        return 2

    try:
        with open(report_path) as fh:
            report = json.load(fh)
    except (OSError, json.JSONDecodeError) as exc:
        print(f"::error::failed to read live-check report at {report_path}: {exc}",
              file=sys.stderr)
        return 1

    stats = report.get("statistics") or {}
    level_counts = stats.get("advice_level_counts") or {}
    highest_counts = stats.get("highest_advice_level_counts") or {}
    violations = int(level_counts.get("violation", 0))
    improvements = int(level_counts.get("improvement", 0))
    informations = int(level_counts.get("information", 0))
    samples = int(stats.get("total_entities", 0) or 0)
    by_type = stats.get("total_entities_by_type") or {}
    msg_counts = stats.get("advice_message_counts") or {}
    coverage = stats.get("registry_coverage")
    no_advice = stats.get("no_advice_count", 0) or 0

    if violations:
        status = "FAIL"
    elif improvements:
        status = "WARN"
    else:
        status = "PASS"

    summary_lines: list[str] = [
        f"## Weaver live-check: {status}",
        "",
    ]
    if by_type:
        parts = ", ".join(f"{v} {k}" for k, v in sorted(by_type.items()))
        summary_lines.append(f"- Samples checked: **{samples}** ({parts})")
    else:
        summary_lines.append(f"- Samples checked: **{samples}**")
    if coverage is not None:
        try:
            summary_lines.append(f"- Registry coverage: **{float(coverage) * 100:.1f}%**")
        except (TypeError, ValueError):
            pass
    summary_lines += [
        "",
        "| Level | Findings | Worst-level samples |",
        "| --- | ---: | ---: |",
        f"| violation | {violations} | {int(highest_counts.get('violation', 0))} |",
        f"| improvement | {improvements} | {int(highest_counts.get('improvement', 0))} |",
        f"| information | {informations} | {int(highest_counts.get('information', 0))} |",
        f"| (no advice) | — | {no_advice} |",
        "",
    ]
    if msg_counts:
        per_level = _findings_by_level(report)
        rendered_any = False
        for lvl, heading in (
            ("violation", "### 🚫 Violations"),
            ("improvement", "### ⚠️ Improvements"),
            ("information", "### ℹ️ Information"),
        ):
            level_msgs = per_level.get(lvl) or {}
            if not level_msgs:
                continue
            rendered_any = True
            top = sorted(level_msgs.items(), key=lambda kv: -kv[1])[:15]
            shown = sum(c for _, c in top)
            total = sum(level_msgs.values())
            suffix = (
                f" (top {len(top)} of {len(level_msgs)} messages,"
                f" {shown} of {total} findings)"
                if len(level_msgs) > len(top)
                else ""
            )
            summary_lines += [
                f"{heading}{suffix}",
                "",
                "| Count | Message |",
                "| ---: | --- |",
            ]
            for msg, count in top:
                cleaned = (msg or "").replace("|", "\\|")
                summary_lines.append(f"| {count} | {cleaned} |")
            summary_lines.append("")

        # Fallback: if per-level parsing produced nothing (older report
        # shapes, missing `samples`, etc.) fall back to the original
        # count-sorted top-N so the summary is never empty.
        if not rendered_any:
            summary_lines += [
                "### Top findings",
                "",
                "| Count | Message |",
                "| ---: | --- |",
            ]
            for msg, count in sorted(msg_counts.items(), key=lambda kv: -kv[1])[:15]:
                cleaned = (msg or "").replace("|", "\\|")
                summary_lines.append(f"| {count} | {cleaned} |")
            summary_lines.append("")

    if violations or improvements or informations:
        summary_lines += [
            "> The full JSON report (every sample and every advisory) is uploaded as a workflow artifact — "
            "look for **`weaver-live-check-report`** in the run's Artifacts section to investigate individual findings.",
            "",
        ]

    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        with open(summary_path, "a", encoding="utf-8") as fh:
            fh.write("\n".join(summary_lines) + "\n")

    output_path = os.environ.get("GITHUB_OUTPUT")
    if output_path:
        with open(output_path, "a", encoding="utf-8") as fh:
            fh.write(f"report-path={report_path}\n")
            fh.write(f"violations={violations}\n")
            fh.write(f"improvements={improvements}\n")
            fh.write(f"informations={informations}\n")
            fh.write(f"samples={samples}\n")

    worst = 0
    if violations:
        worst = max(worst, 3)
    if improvements:
        worst = max(worst, 2)
    if informations:
        worst = max(worst, 1)

    threshold = LEVEL_ORDER[fail_on]
    if threshold == 0 or worst < threshold:
        print(f"OK: highest finding level={worst}, fail-on={fail_on}")
        return 0
    print(
        f"::error::live-check produced findings at level {worst} "
        f"(fail-on={fail_on})",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
