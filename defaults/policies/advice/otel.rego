package advice

import rego.v1

# Use pre-computed sets from jq
attributes_set := data.attributes_set

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
deny contains make_advice(key, advisory, value, message) if {
	not contains(input.name, ".")
	key := "missing_namespace"
	advisory := "improvement"
	value := input.name
	message := "Does not have a namespace"
}

# checks attribute name format
deny contains make_advice(key, advisory, value, message) if {
	not regex.match(name_regex, input.name)
	key := "invalid_format"
	advisory := "violation"
	value := input.name
	message := "Does not match name formatting rules"
}

# checks attribute namespace doesn't collide with existing attributes
deny contains make_advice(key, advisory, value, message) if {
	# Skip if no namespace
	contains(input.name, ".")

	# Get input namespaces
	namespaces := derive_namespaces(input.name)

	# Find collision
	some value in namespaces
	attributes_set[value] != null

	key := "illegal_namespace"
	advisory := "violation"
	message := "Namespace matches existing attribute"
}

# provides advice if the attribute extends an existing namespace
deny contains make_advice(key, advisory, value, message) if {
	# Skip checks first (fail fast)
	contains(input.name, ".") # Must have at least one namespace
	not is_template_type(input.name)
	not is_registry_attribute(input.name)

	# Get input namespaces
	namespaces := derive_namespaces(input.name)

	# Find matches - check keys in set
	matches := [ns | some ns in namespaces; namespaces_to_check_set[ns] != null]
	count(matches) > 0

	# Get the last match (most specific namespace)
	value := matches[count(matches) - 1]

	key := "extends_namespace"
	advisory := "information"
	message := "Extends existing namespace"
}

make_advice(key, advisory, value, message) := {
	"type": "advice",
	"key": key,
	"advisory": advisory,
	"value": value,
	"message": message,
}

# Helper function to check if name is a template type
is_template_type(name) if {
	some template in object.keys(templates_set)
	startswith(name, template)
}

# Helper function to check if name is a registry attribute
is_registry_attribute(name) if {
	attributes_set[name] != null
}
