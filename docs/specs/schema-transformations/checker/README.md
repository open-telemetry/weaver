# Schema transformation checker (prototype)

Validates the transformation files in `../examples` against resolved Weaver registries.

```sh
./build-registries.sh                 # once: registry copies into ./registries
cargo run --release                   # report in target/report/{REPORT,DETAILS}.md, gaps in target/report/gaps/
cargo run --release -- --write-gaps   # also rewrites the top-level `gaps:` block in each example file
```

Args: `[--write-gaps] [examples_dir] [out_dir]`. Env: `TRANSFORM_CHECK_REGISTRIES` (registry root, default `./registries`), `DUMP_DEF=<signal name>` prints a definition as loaded.

## Registries

`build-registries.sh [dest]` extracts read-only copies from local clones with `git archive` (env `SEMCONV`, `SPEC`, `GENAI`, `GENAI_REF`, `SEMCONV_MAIN_REF` override the defaults). Layout: `<dest>/<label>/model` (or `semantic_conventions` for 1.20.0), where `<label>` comes from the `schema_url` (`1.33.0`, `gen-ai-dev-1.42.0-dev`).

- Every `v1.x.y` semantic-conventions tag: example sources/targets, plus intermediate releases used to classify names that are in neither side.
- `1.45.0`: semantic-conventions `origin/main` (examples use 1.45.0 for current main).
- `1.20.0`: opentelemetry-specification `v1.20.0` `semantic_conventions/`, since the semconv repo starts at 1.21. The script inlines the `trace.http.common` attributes (`http.method`, `http.status_code`, `net.sock.*`, ...) into `trace.http.client` / `trace.http.server`; in 1.20 that link existed only in markdown.
- `gen-ai-dev-1.42.0-dev`: semantic-conventions-genai `origin/main`; its `1.44.0` dependency is pointed at the local `1.44.0` copy. Commits used are written to `<dest>/SOURCES`.
- Native gRPC (`grpc-42.42.0`) and the K8s Collector receiver (`1.18.0`) have no registry: only the semconv side is checked and strict is skipped (`# gaps: not checked, ...`).

`constraints:` (1.20-1.25) is rejected by the resolver. The checker rewrites it at load time into `target/patched-registries/<label>`: `include: <group>` becomes `ref`s to the included group's attributes (following its `extends` chain, keeping requirement levels), `any_of` is dropped. Only aggregate groups use `include` (`db.tech`, faas spans).

## How it works

- Schema mode: policies are applied to signal definitions (spans by group id, metrics and events by name), attribute keys only. `exists` needs the key on the definition; `negate exists` matches unless the key is Required; value predicates hold "for some values" when the key exists; span name / status / datapoint value predicates likewise.
- Implied system: per-system DB/RPC spans (`span.db.cassandra.client`, `span.rpc.grpc.call.client`, 1.24 `db.cassandra`, ...) get `db.system[.name]` / `rpc.system[.name]` from the group id (or `extends`), matched against the system enum, and the attribute is added when unlisted. Matchers on that key are then evaluated exactly, and the value is tracked through rename / map / add.
- Ops run in the fixed order across all matching policies; `split` keeps the source key, `add from` needs one `from` key present.
- Loose: matcher / remove / rename.from / split.from keys and matched metric/event names must be in source; add.from / map / convert / discard in source, target, or written by the file; rename.to / split.to / add.target / rename_prefix.to / added metric names in target. Names in neither side but in an intermediate release are reported as such. Keys written and discarded in the same policy are temporaries.
- Target-shaped unchanged: policies applied to non-deprecated target definitions; "definite" when only value-independent matchers are involved, otherwise "value-dependent" (separate section, never a gap).
- Conflicts: same-signal policy pairs, not provably disjoint, that may match a common definition and apply the same op to the same field with different params (also two present keys renamed into one).
- Strict: every non-deprecated target span / metric / event (any stability) in the area scope (`http`, `db`, `rpc`, `gen_ai`+`mcp`, `code` by attribute namespace) must be produced by some source definition after the policies. Pairing uses the transformed definition: metrics / events by name; spans by kind + system value, then id stem (no `span.`/`trace.` prefix or kind suffix). Value-dependent policies fan out per compatible set; a target signal produced only that way is listed as value-dependent, not as a gap. Missing attributes = target required / conditionally required / recommended keys (non-deprecated, non-template) not produced by the best candidate. When the source definition lists no attributes at all (1.37 RPC duration metrics) they are reported as `unknown_attributes` instead.
- `gaps`: computed gaps per file; the declared block is compared with them. `--write-gaps` replaces it in place.

## Known limitations

- No enum narrowing for value matchers beyond the implied system value; span names aren't in V1 definitions.
- System derivation is id-based; spans whose id doesn't name the system stay generic.
- Strict counts development signals, so new target signals with no source counterpart always fail. Code files pass only vacuously (no definition references `code.*`).
- No type / enum-member / unit checks on added values beyond `convert` / `map` notes on target data and instrument/unit diffs on renamed metrics.
- Entities: loose only.
