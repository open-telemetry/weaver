package otel

# Conventions for OTel:
# - `data` holds the current released semconv, which is known to be valid.
# - `input` holds the new candidate semconv version, whose validity is unknown.
#
# Note: `data` and `input` are predefined variables in Rego.

# ========= Violation rules applied on unresolved semconv files =========

# A registry `attribute_group` containing at least one `ref` attribute is
# considered invalid.
violations[violation] {
    group := input.groups[_]
    startswith(group.id, "registry.")
    attr := group.attributes[_]
    attr.ref != null
    violation := {
        "violation": "invalid_registry_ref_attribute",
        "group": group.id,
        "attr": attr.ref,
        "severity": "high",
        "category": "registry"
    }
}

# An attribute whose stability is not `deprecated` but has the deprecated field
# set to true is invalid.
violations[violation] {
    group := input.groups[_]
    attr := group.attributes[_]
    attr.stability != "deprecaded"
    attr.deprecated
    violation := {
        "violation": "invalid_attribute_deprecated_stable",
        "group": group.id,
        "attr": attr.id,
        "severity": "high",
        "category": "attribute"
    }
}

# An attribute cannot be removed from a group that has already been released.
violations[violation] {
    old_group := data.groups[_]
    old_attr := old_group.attributes[_]
    not attr_exists_in_new_group(old_group.id, old_attr.id)

    violation := {
        "violation": "attribute_removed",
        "group": old_group.id,
        "attr": old_attr.id,
        "severity": "high",
        "category": "schema_evolution"
    }
}

# ========= Helper rules =========

# Check if an attribute from the old group exists in the new
# group's attributes
attr_exists_in_new_group(group_id, attr_id) {
    new_group := input.groups[_]
    new_group.id == group_id
    attr := new_group.attributes[_]
    attr.id == attr_id
}