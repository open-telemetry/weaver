package otel

# Conventions for OTel:
# - `data` holds the current released semconv, which is known to be valid.
# - `input` holds the new candidate semconv version, whose validity is unknown.
#
# Note: `data` and `input` are predefined variables in Rego.

# ========= Violation rules applied on unresolved semconv files =========

# A registry `attribute_group` containing at least one `ref` attribute is
# considered invalid.
deny[attr_registry_violation("registry_with_ref_attr", group.id, attr.ref)] {
    group := input.groups[_]
    startswith(group.id, "registry.")
    attr := group.attributes[_]
    attr.ref != null
}

# An attribute whose stability is not `deprecated` but has the deprecated field
# set to true is invalid.
deny[attr_violation("attr_stability_deprecated", group.id, attr.id)] {
    group := input.groups[_]
    attr := group.attributes[_]
    attr.stability != "deprecaded"
    attr.deprecated
}

# An attribute cannot be removed from a group that has already been released.
deny[schema_evolution_violation("attr_removed", old_group.id, old_attr.id)] {
    old_group := data.groups[_]
    old_attr := old_group.attributes[_]
    not attr_exists_in_new_group(old_group.id, old_attr.id)
}


# ========= Helper functions =========

# Check if an attribute from the old group exists in the new
# group's attributes
attr_exists_in_new_group(group_id, attr_id) {
    new_group := input.groups[_]
    new_group.id == group_id
    attr := new_group.attributes[_]
    attr.id == attr_id
}

# Build an attribute registry violation
attr_registry_violation(violation_id, group_id, attr_id) = violation {
    violation := {
        "id": violation_id,
        "type": "semconv_attribute",
        "category": "attrigute_registry",
        "group": group_id,
        "attr": attr_id,
    }
}

# Build an attribute violation
attr_violation(violation_id, group_id, attr_id) = violation {
    violation := {
        "id": violation_id,
        "type": "semconv_attribute",
        "category": "attrigute",
        "group": group_id,
        "attr": attr_id,
    }
}

# Build a schema evolution violation
schema_evolution_violation(violation_id, group_id, attr_id) = violation {
    violation := {
        "id": violation_id,
        "type": "semconv_attribute",
        "category": "schema_evolution",
        "group": group_id,
        "attr": attr_id,
    }
}