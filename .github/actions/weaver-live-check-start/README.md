# `weaver-live-check-start`

Starts an OpenTelemetry Weaver `registry live-check` listener (OTLP/gRPC)
in the background so that an instrumented application can export
telemetry to it for semantic-convention validation.

Pair with [`weaver-live-check-finalize`](../weaver-live-check-finalize/) to
stop the listener, render a step summary, and gate the CI job on
findings.

Linux runners only in v1.

## Example

```yaml
jobs:
  live-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - uses: open-telemetry/weaver/.github/actions/setup-weaver@<ref>

      - uses: open-telemetry/weaver/.github/actions/weaver-live-check-start@<ref>
        id: live-check
        with:
          registry: 'https://github.com/open-telemetry/semantic-conventions/archive/refs/tags/v1.41.1.tar.gz[model]'

      # Build, run, and drive traffic against your app, exporting to the
      # endpoint exposed by the start action.
      - env:
          OTEL_EXPORTER_OTLP_ENDPOINT: ${{ steps.live-check.outputs.otlp-grpc-endpoint }}
        run: |
          ./run-instrumented-app &
          # drive traffic ...
          sleep 6  # let exporters flush
          kill %1

      - uses: open-telemetry/weaver/.github/actions/weaver-live-check-finalize@<ref>
        if: always()
        with:
          fail-on: violation
```

## Inputs

| Name | Required | Default | Description |
|---|---|---|---|
| `registry` | yes | — | Semantic conventions registry to validate against. Any value `weaver registry live-check --registry` accepts. Pin to a released tag for reproducibility. |
| `otlp-grpc-port` | no | `4317` | Port the weaver OTLP/gRPC listener binds to (loopback only). |
| `admin-port` | no | `4320` | Port for weaver admin endpoints (`/health`, `/stop`). |
| `inactivity-timeout` | no | `120` | Seconds the listener stays idle before exiting on its own (safety net). |
| `startup-timeout` | no | `120` | Seconds to wait for `/health` to come up. |
| `diagnostic-format` | no | `gh_workflow_command` | Format passed to weaver's `--diagnostic-format`. |
| `state-dir` | no | `$RUNNER_TEMP/weaver-live-check` | Override if you need multiple instances in the same job (and also override ports). |

## Outputs

| Name | Description |
|---|---|
| `otlp-grpc-endpoint` | `http://127.0.0.1:<otlp-grpc-port>` |
| `admin-endpoint` | `http://127.0.0.1:<admin-port>` |
| `state-dir` | Directory holding run state for the finalize action. |

The start action also exports `$WEAVER_LIVE_CHECK_STATE_DIR` to subsequent
steps in the job, so the finalize action finds it without the caller
needing to re-pass it.

## Behavior

- Verifies `weaver` is on `PATH` (otherwise emits a clear error
  pointing at `setup-weaver`).
- Validates ports and timeouts are positive integers, and that the OTLP
  and admin ports differ.
- Starts `weaver registry live-check` in the background with `--output=http`
  so the report can later be retrieved via the admin `POST /stop`
  response body (no on-disk JSON to coordinate).
- Polls the admin `/health` endpoint until ready, or fails fast if weaver
  exits before becoming ready.
