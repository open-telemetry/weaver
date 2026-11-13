#!/usr/bin/env bash
# Builds the registry copies the checker resolves, from local clones (read-only: git archive).
#
#   ./build-registries.sh [dest]          # dest defaults to ./registries
#
# Env: SEMCONV (default ~/repo/semantic-conventions), SPEC (~/repo/opentelemetry-specification),
#      GENAI (~/repo/semantic-conventions-genai), GENAI_REF (origin/main), SEMCONV_MAIN_REF (origin/main).
set -euo pipefail

DEST="$(cd "$(dirname "${1:-./registries}")" && pwd)/$(basename "${1:-./registries}")"
SEMCONV="${SEMCONV:-$HOME/repo/semantic-conventions}"
SPEC="${SPEC:-$HOME/repo/opentelemetry-specification}"
GENAI="${GENAI:-$HOME/repo/semantic-conventions-genai}"
GENAI_REF="${GENAI_REF:-origin/main}"
SEMCONV_MAIN_REF="${SEMCONV_MAIN_REF:-origin/main}"

mkdir -p "$DEST"

extract() { # repo ref path label
  rm -rf "${DEST:?}/$4"
  mkdir -p "$DEST/$4"
  git -C "$1" archive "$2" "$3" | tar -x -C "$DEST/$4"
}

# Every semconv release: sources/targets of the examples plus intermediates for name lookups.
for tag in $(git -C "$SEMCONV" tag | grep -E '^v1\.[0-9]+\.[0-9]+$'); do
  extract "$SEMCONV" "$tag" model "${tag#v}"
done
# Examples use 1.45.0 for current main.
extract "$SEMCONV" "$SEMCONV_MAIN_REF" model 1.45.0

# 1.20.0 predates the semantic-conventions repo.
extract "$SPEC" v1.20.0 semantic_conventions 1.20.0
# In 1.20 the HTTP spans got trace.http.common attributes through markdown only; inline them.
perl -0 -i -pe 's/(  - id: (trace\.http\.client|trace\.http\.server)\n(?:    .*\n)*?    attributes:\n)/$1      - ref: http.method\n        requirement_level: required\n      - ref: http.status_code\n        requirement_level:\n          conditionally_required: If and only if one was received\/sent.\n      - ref: net.protocol.name\n      - ref: net.protocol.version\n      - ref: http.request_content_length\n      - ref: http.response_content_length\n      - ref: net.sock.peer.addr\n      - ref: net.sock.peer.port\n      - ref: net.sock.peer.name\n      - ref: net.sock.family\n      - ref: user_agent.original\n/g' \
  "$DEST/1.20.0/semantic_conventions/trace/http.yaml"

# GenAI dev registry; its 1.44.0 dependency is redirected to the local copy.
extract "$GENAI" "$GENAI_REF" model gen-ai-dev-1.42.0-dev
perl -i -pe "s#registry_path: https://github.com/open-telemetry/semantic-conventions.git\@v1.44.0\[model\]#registry_path: $DEST/1.44.0/model#" \
  "$DEST/gen-ai-dev-1.42.0-dev/model/manifest.yaml"
echo "genai: $(git -C "$GENAI" rev-parse "$GENAI_REF")" > "$DEST/SOURCES"
echo "semconv main: $(git -C "$SEMCONV" rev-parse "$SEMCONV_MAIN_REF")" >> "$DEST/SOURCES"
cat "$DEST/SOURCES"
