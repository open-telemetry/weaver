#!/bin/bash -eu

cd "$SRC/weaver"

cargo +nightly fuzz build -O --debug-assertions

FUZZ_OUT="fuzz/target/x86_64-unknown-linux-gnu/release"
for f in fuzz/fuzz_targets/*.rs; do
    name="$(basename "${f%.*}")"
    # forge_jinja is excluded from PR runs due to a known upstream minijinja
    # panic (float vs large integer comparison). It still runs in batch/nightly.
    if [ "$name" = "forge_jinja" ] && [ -e "$SRC/weaver/.clusterfuzzlite/.pr_build" ]; then
        continue
    fi
    cp "$FUZZ_OUT/$name" "$OUT/"
done
