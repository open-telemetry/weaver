package live_check_advice

import rego.v1

# checks attribute name contains the word "test"
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.attribute
	value := input.attribute.name
	contains(value, "test")
	advice_type := "contains_test"
	advice_level := "violation"
	message := "Name must not contain 'test'"
}

# checks span name contains the word "test"
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.span
	value := input.span.name
	contains(value, "test")
	advice_type := "contains_test"
	advice_level := "violation"
	message := "Name must not contain 'test'"
}

# checks span_event name contains the word "test"
deny contains make_advice(advice_type, advice_level, value, message) if {
	input.span_event
	value := input.span_event.name
	contains(value, "test")
	advice_type := "contains_test"
	advice_level := "violation"
	message := "Name must not contain 'test'"
}

make_advice(advice_type, advice_level, value, message) := {
	"type": "advice",
	"advice_type": advice_type,
	"advice_level": advice_level,
	"value": value,
	"message": message,
}
