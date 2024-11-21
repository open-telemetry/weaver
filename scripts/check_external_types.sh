#!/bin/bash

# This script checks that each crate in the cargo workspace complies with the
# allowed-external-types.toml file, which defines the types that are allowed to
# be used in the public API.

# Loop through each direct subdirectory in the crates/* directory
for dir in crates/*/; do
  # Skip crate xtask
  if [ "$dir" = "crates/xtask/" ]; then
    continue
  fi
  
  # Check if the public API is compliant with the allowed-external-types.toml
  echo "Checking the public API of $dir"
  cargo +nightly-2024-06-30 check-external-types --all-features --manifest-path "$dir/Cargo.toml" --config "$dir/allowed-external-types.toml" || exit 1
done

echo "The Cargo workspace is compliant with the 'allowed external types' policies."