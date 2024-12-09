#!/bin/bash

REPO_URL="https://github.com/open-telemetry/semantic-conventions.git"
FOLDER="model"
START_TAG="v1.22.0"

# Make the script dir the current dir
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $SCRIPT_DIR

# Functions to compare semantic version tags
version_gte() {
  [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -n1)" = "$1" ]
}

version_gt() {
  version_gte "$1" "$2" && [ "$1" != "$2" ]
}

# Remove versions older than the start tag
for zip_file in ./v*.zip; do
  if [ -f "$zip_file" ]; then
    version="${zip_file##*/}"
    version="${version%.zip}"
    if version_gt "$START_TAG" "$version"; then
      rm -f "$zip_file"
    fi
  fi
done

# Clone the repository
git clone --no-checkout $REPO_URL temp-repo
cd temp-repo

# Get the list of tags and sort them
TAGS=$(git tag | sort -V)

# Loop through each tag and copy the folder as a zip file if it's missing
for TAG in $TAGS; do
  if version_gte $TAG $START_TAG; then
    if [ ! -f "../${TAG}.zip" ]; then
        git checkout $TAG
        zip -r "../${TAG}.zip" $FOLDER
    fi
  fi
done

# Clean up
cd ..
rm -rf temp-repo