default: pre-push

install:
    cargo install cargo-machete
    cargo install cargo-depgraph
    cargo install cargo-edit

pre-push-check:
    cargo update
    cargo machete
    cargo fmt --all
    cargo clippy --workspace --all-features --all-targets -- -D warnings --allow deprecated
    cargo test --all
    cargo doc --workspace --all-features --no-deps --document-private-items

pre-push: pre-push-check
    cargo depgraph --workspace-only | dot -Tsvg > docs/images/dependencies.svg

upgrade:
    cargo upgrade
