default: pre-push

install:
    rustup update 1.93.0
    cargo install cargo-machete@0.9.2 --locked
    cargo install cargo-depgraph@1.6.0 --locked
    cargo install cargo-edit@0.13.10 --locked
    cargo install cargo-check-external-types@0.4.0 --locked
    cargo install git-cliff@2.13.1 --locked
    cargo install cargo-tarpaulin@0.35.4 --locked
    cargo install cargo-nextest@0.9.135 --locked

pre-push-check:
    cargo clean
    cargo update
    cargo machete
    cargo fmt --all
    # [workaround] removed --all-features due to an issue in one of the dependency in Tantity (zstd-safe)
    # [ToDo LQ] Re-enable --all-features once the issue is resolved
    # cargo clippy --workspace --all-features --all-targets -- -D warnings --allow deprecated
    cargo clippy --workspace --all-targets -- -D warnings --allow deprecated
    rm -rf crates/weaver_forge/observed_output/*
    cargo nextest run --all
    cargo xtask history
    # [workaround] removed --all-features due to an issue in one of the dependency in Tantity (zstd-safe)
    # [ToDo LQ] Re-enable --all-features once the issue is resolved
    # cargo doc --workspace --all-features --no-deps --document-private-items
    cargo doc --workspace --no-deps --document-private-items
    cargo deny check licenses

pre-push: install pre-push-check validate-workspace check-external-types
    cargo depgraph --workspace-only --dedup-transitive-deps | dot -Tsvg > docs/images/dependencies.svg

upgrade:
    cargo upgrade

generate:
    docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -v "$(pwd)":/home/weaver/source otel/weaver:v0.23.0 registry generate --registry /home/weaver/source/crates/weaver_live_check/model/ --templates /home/weaver/source/crates/weaver_live_check/templates/ --v2 rust /home/weaver/source/crates/weaver_live_check/src/
    docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -v "$(pwd)":/home/weaver/source otel/weaver:v0.23.0 registry generate --registry /home/weaver/source/crates/weaver_live_check/model/ --templates /home/weaver/source/crates/weaver_live_check/templates/ --v2 markdown /home/weaver/source/crates/weaver_live_check/docs/
    cargo fmt -p weaver_live_check

# Run after `dist generate` to restore scoped GitHub workflow permissions
fix-release-permissions:
    cargo xtask fix-release-permissions

validate-workspace:
    cargo xtask validate

# Run a single fuzz target locally. Requires nightly Rust and cargo-fuzz.
fuzz target="live_check_json" seconds="30":
    cd fuzz && cargo +nightly fuzz run {{target}} -- -max_total_time={{seconds}}

# Run all fuzz targets locally for the given number of seconds each.
fuzz-all seconds="30":
    cd fuzz && failed=0; for target in live_check_json live_check_text semconv_yaml semconv_manifest_yaml forge_config_yaml weaver_config_toml policy_rego; do \
        cargo +nightly fuzz run $target -- -max_total_time={{seconds}} || { echo "CRASH in $target — check fuzz/artifacts/$target/"; failed=1; }; \
    done; exit $failed

check-external-types:
  ##################################################################################
  # TODO Enable this once check-external-types support is available in a version
  #      that is compatible with our current MSRV.
  # See: https://github.com/open-telemetry/weaver/pull/651#issuecomment-2747851893
  ##################################################################################
  #  scripts/check_external_types.sh
  ##################################################################################
