default: pre-push

install:
    cargo install cargo-machete
    cargo install cargo-depgraph
    cargo install cargo-edit
    rustup install nightly-2023-10-10   # used by cargo-check-external-types
    cargo install cargo-check-external-types

pre-push-check:
    rustup update
    cargo clean
    cargo update
    cargo machete
    cargo fmt --all
    cargo clippy --workspace --all-features --all-targets -- -D warnings --allow deprecated
    cargo test --all
    cargo doc --workspace --all-features --no-deps --document-private-items
    cargo deny check licenses

pre-push: pre-push-check validate-workspace check-external-types
    cargo depgraph --workspace-only | dot -Tsvg > docs/images/dependencies.svg

upgrade:
    cargo upgrade

validate-workspace:
    cargo xtask validate

check-external-types:
    scripts/check_external_types.sh