package advice

import rego.v1

# checks attribute name format
deny contains make_advice(key, advisory, value, message) if {
	not regex.match(name_regex, input.name)
	key := "invalid_format"
	advisory := "violation"
	value := input.name
	message := "Does not match name formatting rules"
}

# checks attribute has a namespace
deny contains make_advice(key, advisory, value, message) if {
	not contains(input.name, ".")
	key := "missing_namespace"
	advisory := "improvement"
	value := input.name
	message := "Does not have a namespace"
}

# checks attribute namespace doesn't collide with existing attributes
deny contains make_advice(key, advisory, value, message) if {
	some attr in object.keys(data.semconv_attributes)

	namespaces := [ns |
		some i, _ in split(input.name, ".")
		i > 0
		ns := concat(".", array.slice(split(input.name, "."), 0, i))
	]

	some value in namespaces
	attr == value

	key := "illegal_namespace"
	advisory := "violation"
	message := "Namespace matches existing attribute"
}

# provides advice if the attribute extends an existing namespace
deny contains make_advice(key, advisory, value, message) if {
	# Skip this rule if the attribute is a template type
	not is_template_type(input.name)

	# Skip this rule if the attribute is a registry attribute
	not is_registry_attribute(input.name)

	namespaces_to_check := {ns |
		some attr_name, _ in data.semconv_attributes
		some i, _ in split(attr_name, ".")
		i > 0
		ns := concat(".", array.slice(split(attr_name, "."), 0, i))
	}

	namespaces := [ns |
		some i, _ in split(input.name, ".")
		i > 0
		ns := concat(".", array.slice(split(input.name, "."), 0, i))
	]

	count(namespaces) > 0

	# Find all matching namespaces
	matches := [ns_value |
		some ns_value in namespaces
		some ns in namespaces_to_check
		ns_value == ns
	]

	# Only continue if there are matches
	count(matches) > 0

	# Set value to the last match
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

# not valid: '1foo.bar', 'foo.bar.', 'foo.bar_', 'foo..bar', 'foo._bar' ...
# valid: 'foo.bar', 'foo.1bar', 'foo.1_bar'
name_regex := "^[a-z][a-z0-9]*([._][a-z0-9]+)*$"

# Helper function to check if name is a template type
is_template_type(name) if {
	some templates in object.keys(data.semconv_templates)
	startswith(name, templates)
}

# Helper function to check if name is a registry attribute
is_registry_attribute(name) if {
	some attr in object.keys(data.semconv_attributes)
	name == attr
}
