package otel

# Conventions for OTel:
# - `data` holds the current released semconv, which is known to be valid.
# - `input` holds the new candidate semconv version, whose validity is unknown.
#
# Note: `data` and `input` are predefined variables in Rego.

# ========= Violation rules applied on unresolved semconv files =========

# A registry `attribute_group` containing at least one `ref` attribute is
# considered invalid.
detect[violation_registry("invalid_registry_ref_attribute", group.id, attr.ref)] {
    group := input.groups[_]
    startswith(group.id, "registry.")
    attr := group.attributes[_]
    attr.ref != null
}

# An attribute whose stability is not `deprecated` but has the deprecated field
# set to true is invalid.
detect[violation_attribute("invalid_attribute_deprecated_stable", group.id, attr.id)] {
    group := input.groups[_]
    attr := group.attributes[_]
    attr.stability != "deprecaded"
    attr.deprecated
}

# An attribute cannot be removed from a group that has already been released.
detect[violation_schema_evolution("attribute_removed", old_group.id, old_attr.id)] {
    old_group := data.groups[_]
    old_attr := old_group.attributes[_]
    not attr_exists_in_new_group(old_group.id, old_attr.id)
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

# Build a schema evolution violation
violation_schema_evolution(violation_id, group_id, attr_id) = violation {
    violation := {
        "violation": violation_id,
        "group": group_id,
        "attr": attr_id,
        "severity": "high",
        "category": "schema_evolution"
    }
}

# Build a registry violation
violation_registry(violation_id, group_id, attr_id) = violation {
    violation := {
        "violation": violation_id,
        "group": group_id,
        "attr": attr_id,
        "severity": "high",
        "category": "registry"
    }
}

# Build a attribute violation
violation_attribute(violation_id, group_id, attr_id) = violation {
    violation := {
        "violation": violation_id,
        "group": group_id,
        "attr": attr_id,
        "severity": "high",
        "category": "attribute"
    }
}