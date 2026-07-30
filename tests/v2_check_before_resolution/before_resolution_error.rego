package before_resolution

import rego.v1

# This needs to exist so we get a warning that before resolution does not work in V2.
deny contains invalid_attr_violation("invalid_metric_attr", metric.name, attr.key) if {
	metric := input.metrics[_]
	attr := metric.attributes[_]
	attr.key == "my.attr"
}

invalid_attr_violation(violation_id, group_id, attr_id) := violation if {
	violation := {
		"id": violation_id,
		"type": "semconv_attribute",
		"category": "attribute",
		"group": group_id,
		"attr": attr_id,
	}
}