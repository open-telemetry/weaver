package semconv
import rego.v1

# Semantic Convention Registry Helpers
#
# This file contains a set of common rules and functions to process
# semantic convention registries. It's designed to work with both current
# and baseline (previous version) registries for compatibility checks.

# Input Expectations:
# 1. Current Registry:
#    - Accessible via `input.groups`
#    - Specified by the `--registry` flag when running Weaver
#
# 2. Baseline Registry (optional):
#    - Accessible via `data.groups` if provided
#    - Specified by the `--baseline-registry` flag when running Weaver
#    - Represents the previous version of the registry for compatibility checks

# Define baseline and current groups
baseline_groups := data.groups          # Baseline registry groups (if provided)
groups := input.groups                  # Current registry groups

# Filter "registry" groups
# These comprehensions create arrays of groups whose IDs start with "registry."
# for both baseline and current registries.
registry_baseline_groups := [g | g := baseline_groups[_]; startswith(g.id, "registry.")]
registry_groups := [g | g := input.groups[_]; startswith(g.id, "registry.")]

# Collect all attribute names from the baseline registry
# This set comprehension gathers all attribute names from groups
# in the baseline registry
baseline_attributes := {attr.name |
    some g in baseline_groups
    some attr in g.attributes
}

# Collect all registry attribute names from the baseline registry
# This set comprehension gathers all attribute names from groups
# in the baseline registry that start with "registry."
registry_baseline_attributes := {attr.name |
    some g in registry_baseline_groups
    some attr in g.attributes
}

# Collect all attribute names from the current registry
# Similar to baseline_attributes, but for the current groups
attributes := {attr.name |
    some g in groups
    some attr in g.attributes
}

# Collect all registry attribute names from the current registry
# Similar to baseline_attributes, but for the current registry groups
registry_attributes := {attr.name |
    some g in registry_groups
    some attr in g.attributes
}

# Map attribute names to their group IDs in the baseline registry
# This object comprehension creates a mapping where:
# - Keys are attribute names
# - Values are the IDs of the groups containing these attributes
# Only considers groups whose IDs start with "registry."
baseline_group_ids_by_attribute := {attr.name: g.id |
    some g in registry_baseline_groups
    some attr in g.attributes
}
