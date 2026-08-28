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

# Entity Associations (v2)
#
# A signal names the entities it belongs to with `entity_associations`. An
# association can be a tree. A `one_of` or `all_of` node contains more
# associations, and only a leaf names an entity. A leaf gives the entity type
# and, in `provenance.source`, the registry that defines it. A leaf with no
# provenance names an entity of the registry under check.
#
# These functions read the materialized v2 registry, which an
# `after_resolution` policy gets as `input`.

# Every entity reference in an association, or in a list of associations. The
# walk covers every level of the tree, so a policy does not have to.
entity_refs(association) := {node |
    walk(association, [_, node])
    is_object(node)
    node.type
}

# The registry that defines the entity a reference names.
defining_registry(ref) := input if {
    not ref.provenance.source
}

defining_registry(ref) := input.dependencies[ref.provenance.source]

# The entity definition a reference names. A reference names an entity type or
# the id of an entity refinement. The entity type wins, as it does in weaver.
lookup_entity(ref) := entity if {
    registry := defining_registry(ref)
    matches := array.concat(
        [e | some e in registry.registry.entities; e.type == ref.type],
        [r | some r in registry.refinements.entities; r.id == ref.type],
    )
    entity := matches[0]
}
