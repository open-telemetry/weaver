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

pre-push: pre-push-check check-readme
    cargo depgraph --workspace-only | dot -Tsvg > docs/images/dependencies.svg

upgrade:
    cargo upgrade

# Check for the presence of README.md files in every crate defined in the
# workspace.
check-readme:
    #!/bin/bash

    # Navigate to the crates directory
    cd crates || exit

    # Loop through each direct subdirectory in the crates/* directory
    for dir in */; do
      # Check if README.md exists in the subdirectory
      if [ ! -f "$dir/README.md" ]; then
        echo "README.md missing in $dir"
        cd - > /dev/null  # Return to the original directory
        exit 1
      fi
    done

    cd - > /dev/null  # Return to the original directory
    echo "Checked for the presence of README.md files in the workspace. All crates have a README.md file."