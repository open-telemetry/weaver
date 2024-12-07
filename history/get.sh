#!/bin/bash

REPO_URL="https://github.com/open-telemetry/semantic-conventions.git"
FOLDER="model"
START_TAG="v1.21.0"

# Make the script dir the current dir
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $SCRIPT_DIR

# Function to compare semantic version tags
version_gt() {
  [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -n1)" = "$1" ] && [ "$1" != "$2" ]
}

# Determine the highest version already present
# Remove versions older than the start tag
HIGHEST_VERSION="$START_TAG"
for dir in v*/; do
  if [ -d "$dir" ]; then
    version="${dir%/}"
    if version_gt "$version" "$HIGHEST_VERSION"; then
      HIGHEST_VERSION="$version"
    fi
    if version_gt "$START_TAG" "$version"; then
      rm -rf "$dir"
    fi
  fi
done

echo "Highest version already present: $HIGHEST_VERSION"

# Clone the repository
git clone --no-checkout $REPO_URL temp-repo
cd temp-repo

# Get the list of tags and sort them
TAGS=$(git tag | sort -V)

# Print the list of tags
echo "Tags in the repository:"
echo "$TAGS"

# Loop through each tag and copy the folder
for TAG in $TAGS; do
  if version_gt $TAG $HIGHEST_VERSION; then
    echo "Processing tag: $TAG"
    git checkout $TAG
    mkdir -p "../$TAG"
    cp -r $FOLDER "../$TAG/"
  fi
done

# Clean up
cd ..
rm -rf temp-repo