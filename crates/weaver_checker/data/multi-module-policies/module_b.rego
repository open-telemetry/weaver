package after_resolution

import rego.v1

# Module B: uses a function with an else clause (the pattern that triggered
# the regorus 0.9.x regression when multiple modules share the same package)

# Helper to create violations
member_violation(description, group_id, attr_id) = v if {
    v := {
        "id": description,
        "type": "semconv_attribute",
        "category": "member_check",
        "group": group_id,
        "attr": attr_id,
    }
}

# Function with else clause - this pattern caused binding plan errors in regorus 0.9.x
# when multiple modules share the same package.
is_property_set(obj, property) = true if {
    obj[property] != null
} else = false

deny contains member_violation(description, group.id, attr.name) if {
    group := input.groups[_]
    startswith(group.id, "registry.")
    attr := group.attributes[_]
    member := attr.type.members[_]
    not is_property_set(member, "deprecated")

    collisions := [m |
        m := attr.type.members[_]
        not is_property_set(m, "deprecated")
        m.value == member.value
    ]
    count(collisions) > 1

    description := sprintf("Duplicate member value '%s' in '%s'", [member.value, attr.name])
}
