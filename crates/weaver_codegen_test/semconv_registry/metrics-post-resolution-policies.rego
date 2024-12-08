package after_resolution

# Example of rules that will be applied on resolved semconv files

# Detect `system.cpu.logical_number` attribute and consider it invalid.
# This is just an example for testing purposes.
deny[invalid_attr_violation("invalid_metric_attr", group.id, attr.name)] {
    group := input.groups[_]
    attr := group.attributes[_]
    attr.name == "system.cpu.logical_number"
}

invalid_attr_violation(violation_id, group_id, attr_id) = violation {
    violation := {
        "id": violation_id,
        "type": "semconv_attribute",
        "category": "attribute",
        "group": group_id,
        "attr": attr_id,
    }
}