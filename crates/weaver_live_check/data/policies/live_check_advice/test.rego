package live_check_advice

import rego.v1

# checks attribute name contains the word "test"
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
	input.sample.attribute
	advice_context := {
		"attribute_name": input.sample.attribute.name
	}
	contains(input.sample.attribute.name, "test")
	advice_type := "contains_test"
	advice_level := "violation"
	message :=  sprintf("Attribute name must not contain 'test', but was '%s'", [input.sample.attribute.name])
}

# checks span name contains the word "test"
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
	input.sample.span
	advice_context := {
		"span_name": input.sample.span.name
	}
	contains(input.sample.span.name, "test")
	advice_type := "contains_test"
	advice_level := "violation"
	message :=  sprintf("Span name must not contain 'test', but was '%s'", [input.sample.span.name])
}

# checks span status message contains the word "test"
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
	input.sample.span
	advice_context := {
		"span_status_message": input.sample.span.status.message
	}
	contains(input.sample.span.status.message, "test")
	advice_type := "contains_test_in_status"
	advice_level := "violation"
	message :=  sprintf("Span status message must not contain 'test', but was '%s'", [input.sample.span.status.message])
}

# checks span_event name contains the word "test"
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
	input.sample.span_event
	advice_context := {
		"span_event_name": input.sample.span_event.name
	}
	contains(input.sample.span_event.name, "test")
	advice_type := "contains_test"
	advice_level := "violation"
	message :=  sprintf("Span event name must not contain 'test', but was '%s'", [input.sample.span_event.name])
}

# This example shows how to use the registry_group provided in the input.
# If the metric's unit is "By" the value in this data-point must be an integer.
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
	input.sample.number_data_point
	input.registry_group.unit == "By"
	input.sample.number_data_point.value != floor(input.sample.number_data_point.value) # not a good type check, but serves as an example
	advice_context := {
		"data_point_value": input.sample.number_data_point.value
	}
	advice_type := "invalid_data_point_value"
	advice_level := "violation"
	message := sprintf("Metric with unit 'By' must have an integer value, but was '%v'", [input.sample.number_data_point.value])
}

# As above, but for exemplars which are nested two levels deep.
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
	input.sample.exemplar
	advice_context := {
		"exemplar_value": input.sample.exemplar.value
	}
	input.registry_group.unit == "s"
	input.sample.exemplar.value < 1.0
	advice_type := "low_value"
	advice_level := "information"
	message := sprintf("This is a low number of seconds: %v", [input.sample.exemplar.value])
}

make_advice(advice_type, advice_level, advice_context, message) := {
	"type": "advice",
	"advice_type": advice_type,
	"advice_level": advice_level,
	"advice_context": advice_context,
	"message": message,
}
