#!/bin/bash -eu

cd "$SRC/weaver"

cargo +nightly fuzz build -O --debug-assertions

FUZZ_OUT="fuzz/target/x86_64-unknown-linux-gnu/release"
for f in fuzz/fuzz_targets/*.rs; do
    cp "$FUZZ_OUT/$(basename "${f%.*}")" "$OUT/"
done
