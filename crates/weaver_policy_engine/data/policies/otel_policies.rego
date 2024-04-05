package otel

# A registry attribute groups containing at least one `ref` attribute is considered invalid.
violations[violation] {
    group := data.groups[_]
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

# An attribute marked as stable and deprecated is invalid.
violations[violation] {
    group := data.groups[_]
    attr := group.attributes[_]
    attr.stability == "stable"
    attr.deprecated
    violation := {
        "violation": "invalid_attribute_deprecated_stable",
        "group": group.id,
        "attr": attr.id,
        "severity": "high",
        "category": "attribute"
    }
}

# other violations rules here...

