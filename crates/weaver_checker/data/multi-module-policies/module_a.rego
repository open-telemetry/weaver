package after_resolution

import rego.v1

# Module A: defines basic validation rules

# Helper to create violations
violation(description, group_id) = v if {
    v := {
        "id": description,
        "type": "semconv_attribute",
        "category": "test",
        "group": group_id,
        "attr": "",
    }
}

deny contains violation(description, group.id) if {
    group := input.groups[_]
    not group.stability
    description := sprintf("Group '%s' missing stability", [group.id])
}
