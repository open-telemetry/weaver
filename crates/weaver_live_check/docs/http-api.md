# Driving live-check over HTTP

`weaver registry live-check --input-source otlp` receives OTLP over gRPC and checks it.
In CI, a script usually controls the run: start weaver, run the code under test, then
collect the report. The admin port serves four endpoints for this. With `--output=http`,
the report is also served on the admin port.

## Sequence

```mermaid
sequenceDiagram
    participant CI as CI script
    participant W as weaver live-check
    participant App as App under test

    CI->>W: start with --input-source otlp --output http
    activate W
    loop until 200
        CI->>W: GET /health
    end
    W-->>CI: 200 {"status":"ready"}

    CI->>App: run with OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317
    activate App
    App->>W: OTLP export (gRPC)
    Note over W: queue the export, then acknowledge
    W-->>App: ack
    App->>W: OTLP export (gRPC)
    W-->>App: ack
    App->>App: flush and exit
    deactivate App

    CI->>W: POST /stop
    Note over W: stop receiving, check the queued exports, render the report
    W-->>CI: 200 {"state":"stopped","report":true}

    CI->>W: GET /report
    W-->>CI: 200 the report (Content-Type from --format)

    CI->>W: POST /shutdown
    W-->>CI: 200 {"state":"shutting_down"}
    deactivate W
    Note over W: process exits
```

## What the sequence guarantees

- **Reading the report is separate from exiting.** `GET /report` is served until
  `POST /shutdown`, and shutdown waits for in-flight responses to complete. A large or
  slowly read report is not truncated.
- **`/stop` returns after the report is ready.** The client does not poll. `/stop` has no
  server-side timeout, so set one in the client.
- **Acknowledged exports are checked.** Weaver acknowledges an export only after it is in
  the queue, and `/stop` adds its stop request to the end of the same queue. An export
  acknowledged before `/stop` is sent is included in the report.

## What it does not guarantee

Weaver checks only the exports it received and acknowledged before `/stop`. It cannot
detect telemetry that the app did not send. Data is missing from the report when:

- The app exits without flushing. Batch processors hold data until they export it. Call
  the SDK's `shutdown` or `force_flush` before the app exits.
- An export is still in progress when `/stop` is sent. Wait for the app to exit before
  calling `/stop`.
- An exporter times out. The queue holds 100 exports. If the checker falls behind, an
  export waits for a free slot, and the exporter can time out before it is acknowledged.
  That export is not queued. It is checked only if the SDK retries it before `/stop`.
- An export arrives after the stop. Weaver refuses it with `UNAVAILABLE`.

## Endpoints

| Method | Path | Returns |
| --- | --- | --- |
| `GET` | `/health` | `200 {"status":"ready"}` when the listener is up. |
| `POST` | `/stop` | Stops receiving and waits for the report. Returns `200 {"state":"stopped","report":true\|false}`. `report` is `true` with `--output=http`. Later calls return the same response. |
| `GET` | `/report` | The report, with `--output=http`. Returns `409` while still receiving and after shutdown starts. |
| `POST` | `/shutdown` | Returns `200 {"state":"shutting_down"}`, then the process exits. If the run is still receiving, stops it first. |

Weaver logs each request to stderr.

## Shell script

```bash
#!/usr/bin/env bash
set -euo pipefail

weaver registry live-check -r model --input-source otlp \
  --otlp-grpc-port 4317 --admin-port 4320 \
  --format json --output http &
weaver_pid=$!

until curl -fsS http://127.0.0.1:4320/health >/dev/null 2>&1; do sleep 0.5; done

# The app must flush its telemetry before it exits.
OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317 ./run-my-tests

curl -fsS --max-time 300 -X POST http://127.0.0.1:4320/stop
curl -fsS http://127.0.0.1:4320/report -o report.json
curl -fsS -X POST http://127.0.0.1:4320/shutdown
wait "$weaver_pid"
```

The [`weaver-live-check-start`](../../../.github/actions/weaver-live-check-start/) and
[`weaver-live-check-stop`](../../../.github/actions/weaver-live-check-stop/) GitHub actions
run the same sequence and also grade the report.

## Stopping and exiting

With `--output=http`, the client controls the run. Weaver does not stop or exit unless the
client or a signal tells it to:

- `--inactivity-timeout` is ignored. If it is set, weaver logs a warning.
- After `/stop`, weaver waits for `/shutdown` with no time limit.
- The first `SIGINT` or `SIGHUP` stops the run. The second ends the process. The report is
  served only on `/report`, so it is lost if the process exits before a client reads it.

Without `--output=http`, the report goes to stdout or to `--output <dir>`. `/stop` stops
the run, weaver writes the report, and the process exits. Inactivity and signals work as
usual.

## Ports

`--otlp-grpc-port` and `--admin-port` must be different. Set either to `0` to use a free
port. The startup log shows the bound addresses. Both ports bind to
`--otlp-grpc-address`, which defaults to the loopback interface.
