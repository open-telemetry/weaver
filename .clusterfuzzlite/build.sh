#!/bin/bash -eu

cd "$SRC/weaver"

cargo +nightly fuzz build --manifest-path fuzz/Cargo.toml

FUZZ_OUT="fuzz/target/x86_64-unknown-linux-gnu/release"
cp "$FUZZ_OUT/live_check_json"       "$OUT/"
cp "$FUZZ_OUT/live_check_text"       "$OUT/"
cp "$FUZZ_OUT/semconv_yaml"          "$OUT/"
cp "$FUZZ_OUT/semconv_manifest_yaml" "$OUT/"
cp "$FUZZ_OUT/forge_config_yaml"     "$OUT/"
cp "$FUZZ_OUT/weaver_config_toml"    "$OUT/"
cp "$FUZZ_OUT/policy_rego"           "$OUT/"
