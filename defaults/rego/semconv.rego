package semconv
import rego.v1

# This file contains a set of common rules/functions to process semantic convention registries.

# Expected inputs:
# - The semconv registry pointed to by `--registry` will be accessible via `input.groups`.
# - If Weaver is run with the `--baseline-registry`, then `data.groups` will contain the groups
#   of the baseline semconv registry, or in other words, the previous version of the registry used
#   in the context of a compatibility check, for example.

baseline_groups := data.groups
groups := input.groups

registry_baseline_groups := [g | g := baseline_groups[_]; startswith(g.id, "registry.")]
registry_groups := [g | g := input.groups[_]; startswith(g.id, "registry.")]

# Collect all attributes from previous groups that are prefixed by "registry."
baseline_attributes := {attr.name |
    some g in registry_baseline_groups
    some attr in g.attributes
}

# Collect all attributes from current groups that are prefixed by "registry."
attributes := {attr.name |
    some g in registry_groups
    some attr in g.attributes
}

# This rule is essentially creating a map where each entry associates an attribute
# name with the ID of the group from the baseline registry that contains this attribute.
# It only considers groups whose IDs start with “registry.”.
baseline_group_ids_by_attribute := {attr.name: g.id |
    some g in registry_baseline_groups
    some attr in g.attributes
}
