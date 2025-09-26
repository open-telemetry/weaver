default: pre-push

install:
    cargo install cargo-machete
    cargo install cargo-depgraph
    cargo install cargo-edit
    rustup install nightly-2024-10-29   # used by cargo-check-external-types
    cargo install cargo-check-external-types
    cargo install git-cliff
    cargo install cargo-tarpaulin
    cargo install cargo-nextest --locked

pre-push-check:
    rustup update
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

pre-push: pre-push-check validate-workspace check-external-types
    cargo depgraph --workspace-only --dedup-transitive-deps | dot -Tsvg > docs/images/dependencies.svg

upgrade:
    cargo upgrade

validate-workspace:
    cargo xtask validate

check-external-types:
  ##################################################################################
  # TODO Enable this once check-external-types support is available in a version
  #      that is compatible with our current MSRV.
  # See: https://github.com/open-telemetry/weaver/pull/651#issuecomment-2747851893
  ##################################################################################
  #  scripts/check_external_types.sh
  ##################################################################################
