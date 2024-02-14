#!/bin/bash

# This script checks that the rules below are properly followed for each crate
# in the cargo workspace.
# - Each crate must have a README.md file.
# - Each crate name must start with "weaver_" to avoid conflicts with other
#   crates.
# - Each crate must have an allowed-external-types.toml file defining the types
#   that are allowed to be used in the public API.

# Navigate to the crates directory (workspace)
cd crates || exit

# Loop through each direct subdirectory in the crates/* directory
for dir in */; do
  # Check if the crate name starts with "weaver_"
  if [[ ! "$dir" =~ ^weaver_ ]]; then
    echo "Crate name does not start with 'weaver_': $dir"
    cd - > /dev/null  # Return to the original directory
    exit 1
  fi

  # Check if README.md exists in the crate
  if [ ! -f "$dir/README.md" ]; then
    echo "'README.md' missing in $dir"
    cd - > /dev/null  # Return to the original directory
    exit 1
  fi

  # Check if allowed-external-types.toml exists in the crate
  if [ ! -f "$dir/allowed-external-types.toml" ]; then
    echo "'allowed-external-types.toml' missing in $dir"
    cd - > /dev/null  # Return to the original directory
    exit 1
  fi
done

cd - > /dev/null  # Return to the original directory
echo "The Cargo workspace is compliant with the project policies."