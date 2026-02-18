default: pre-push

install:
    rustup update 1.93.0
    cargo install cargo-machete@0.9.1 --locked
    cargo install cargo-depgraph@1.6.0 --locked
    cargo install cargo-edit@0.13.8 --locked
    cargo install cargo-check-external-types@0.4.0 --locked
    cargo install git-cliff@2.12.0 --locked
    cargo install cargo-tarpaulin@0.35.1 --locked
    cargo install cargo-nextest@0.9.127 --locked

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
