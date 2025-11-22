package comparison_after_resolution

import rego.v1

deny contains bcompat_violation(description) if {
	metric := input.signals.metrics[_]
    baseline_metric := data.signals.metrics[_]
    metric.name == baseline_metric.name
    baseline_attributes := { attr.key |
        some attr in baseline_metric.attributes
        attr.stability == "stable"
    }
    new_attributes := { attr.key |
        some attr in metric.attributes
        attr.stability == "stable"
    }
    missing_attributes := baseline_attributes - new_attributes
    count(missing_attributes) > 0
    description := sprintf("Metric '%s' cannot change required/recommended attributes (missing '%s')", [metric.name, missing_attributes])
}

bcompat_violation(description) := violation if {
	violation := {
		"id": description,
		"type": "semconv_attribute",
		"category": "attribute",
		"group": "",
		"attr": "",
	}
}