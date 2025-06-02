package live_check_advice

import rego.v1

# Use pre-computed sets from jq
attributes_set := data.attributes_set

deprecated_attributes_set := data.deprecated_attributes_set

templates_set := data.templates_set

namespaces_to_check_set := data.namespaces_to_check_set

name_regex := "^[a-z][a-z0-9]*([._][a-z0-9]+)*$"

derive_namespaces(name) := [
concat(".", array.slice(parts, 0, i)) |
	parts := split(name, ".")
	count(parts) > 1 # Only derive namespaces if there are at least 2 parts

	# Stop at count(parts)-1 to exclude the full attribute name
	some i in numbers.range(1, count(parts) - 1)
]

# checks attribute has a namespace
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.sample.attribute
	not contains(input.sample.attribute.name, ".")
	advice_type := "missing_namespace"
	advice_level := "improvement"
	value := input.sample.attribute.name
	message := "Does not have a namespace"
}

# checks attribute name format
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.sample.attribute
	not regex.match(name_regex, input.sample.attribute.name)
	advice_type := "invalid_format"
	advice_level := "violation"
	value := input.sample.attribute.name
	message := "Does not match name formatting rules"
}

# checks metric name format
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.sample.metric
	not regex.match(name_regex, input.sample.metric.name)
	advice_type := "invalid_format"
	advice_level := "violation"
	value := input.sample.metric.name
	message := "Does not match name formatting rules"
}

# checks attribute namespace doesn't collide with existing attributes
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.sample.attribute

	# Skip if no namespace
	contains(input.sample.attribute.name, ".")

	# Get input namespaces
	namespaces := derive_namespaces(input.sample.attribute.name)

	# Find collision
	some value in namespaces
	attributes_set[value]
	not deprecated_attributes_set[value]

	advice_type := "illegal_namespace"
	advice_level := "violation"
	message := "Namespace matches existing attribute"
}

# provides advice if the attribute extends an existing namespace
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.sample.attribute

	# Skip checks first (fail fast)
	contains(input.sample.attribute.name, ".") # Must have at least one namespace
	not is_template_type(input.sample.attribute.name)
	not attributes_set[input.sample.attribute.name]

	# Get input namespaces
	namespaces := derive_namespaces(input.sample.attribute.name)

	# Find matches - check keys in set
	matches := [ns | some ns in namespaces; namespaces_to_check_set[ns]]
	count(matches) > 0

	# Get the last match (most specific namespace)
	value := matches[count(matches) - 1]

	advice_type := "extends_namespace"
	advice_level := "information"
	message := "Extends existing namespace"
}

make_advice(advice_type, advice_level, value, message) := {
	"type": "advice",
	"advice_type": advice_type,
	"advice_level": advice_level,
	"value": value,
	"message": message,
}

# Helper function to check if name is a template type
is_template_type(name) if {
	some template in object.keys(templates_set)
	startswith(name, template)
}
