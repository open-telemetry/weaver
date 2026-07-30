# `weaver-live-check-stop`

> **Note:** Despite the name, this action does **more than just stop**
> the listener. It also fetches the live-check report, renders a job
> step summary, exposes counts as outputs, and (by default) **fails the
> job** when findings reach the configured severity. Named `stop` for
> symmetry with `weaver-live-check-start` and because it calls weaver's
> admin `/stop` endpoint.

Pair with [`weaver-live-check-start`](../weaver-live-check-start/). See
that action's README for an end-to-end example.

Linux runners only in v1.

## Inputs

| Name | Required | Default | Description |
|---|---|---|---|
| `fail-on` | no | `violation` | Lowest finding level that should fail the job: `violation` \| `improvement` \| `information` \| `none`. Start with `none` when first adopting; tighten once existing findings are addressed. |
| `state-dir` | no | `$WEAVER_LIVE_CHECK_STATE_DIR` | State directory produced by `weaver-live-check-start`. |
| `stop-timeout` | no | `30` | Seconds to wait for weaver to flush and exit cleanly. |
| `upload-report` | no | `true` | When `true`, upload the captured live-check JSON report as a workflow artifact so the full per-sample, per-advisory detail is downloadable from the run page. |
| `report-artifact-name` | no | `weaver-live-check-report` | Artifact name. Override when running multiple live-check instances in the same job. |

## Outputs

| Name | Description |
|---|---|
| `report-path` | Path to the captured live-check JSON report. |
| `violations` | Number of violation-level findings. |
| `improvements` | Number of improvement-level findings. |
| `informations` | Number of information-level findings. |
| `samples` | Number of telemetry samples weaver received. |

## Behavior

- Validates `fail-on`, `stop-timeout`, and `upload-report` inputs.
- POSTs to weaver's admin `/stop`, capturing the in-memory report body to
  `state-dir/live_check.json`.
- Waits for the weaver process to exit cleanly (up to `stop-timeout`);
  hard-kills if it does not.
- Parses the report with `parse-report.py` (a Python script bundled
  alongside this action) and writes a markdown summary to
  `$GITHUB_STEP_SUMMARY` — including counts per level, registry
  coverage, and one findings table **per severity** (Violations,
  Improvements, Information) so the most actionable issues stay at
  the top regardless of count.
- Uploads the full JSON report as a workflow artifact (named
  `weaver-live-check-report` by default) so users can drill into the
  exact failing samples and advisories from the run page. Disable with
  `upload-report: false`.
- Sets action outputs for `report-path`, `violations`, `improvements`,
  `informations`, and `samples`.
- Exits non-zero (failing the step) when the worst finding level meets
  or exceeds the `fail-on` threshold. The artifact is still uploaded in
  this case (the upload step runs with `if: always()`).

Call this action with `if: always()` in your workflow so that the
listener is shut down and the report uploaded even if a preceding
project-specific step (build, app start, traffic) fails.
