package live_check_advice

import rego.v1

# checks attribute name contains the word "test"
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.sample.attribute
	value := input.sample.attribute.name
	contains(value, "test")
	advice_type := "contains_test"
	advice_level := "violation"
	message := "Name must not contain 'test'"
}

# checks span name contains the word "test"
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.sample.span
	value := input.sample.span.name
	contains(value, "test")
	advice_type := "contains_test"
	advice_level := "violation"
	message := "Name must not contain 'test'"
}

# checks span status message contains the word "test"
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.sample.span
	value := input.sample.span.status.message
	contains(value, "test")
	advice_type := "contains_test_in_status"
	advice_level := "violation"
	message := "Status message must not contain 'test'"
}

# checks span_event name contains the word "test"
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.sample.span_event
	value := input.sample.span_event.name
	contains(value, "test")
	advice_type := "contains_test"
	advice_level := "violation"
	message := "Name must not contain 'test'"
}

# This example shows how to use the registry_group provided in the input.
# If the metric's unit is "By" the value in this data-point must be an integer.
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.sample.number_data_point
	value := input.sample.number_data_point.value
	input.registry_group.unit == "By"
	value != floor(value) # not a good type check, but serves as an example
	advice_type := "invalid_data_point_value"
	advice_level := "violation"
	message := "Value must be an integer when unit is 'By'"
}

# As above, but for exemplars which are nested two levels deep.
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.sample.exemplar
	value := input.sample.exemplar.value
	input.registry_group.unit == "s"
	value < 1.0
	advice_type := "low_value"
	advice_level := "information"
	message := "This is a low number of seconds"
}

make_advice(advice_type, advice_level, value, message) := {
	"type": "advice",
	"advice_type": advice_type,
	"advice_level": advice_level,
	"value": value,
	"message": message,
}
