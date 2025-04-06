package live_check_advice

import rego.v1

# checks attribute name contains the word "test"
deny contains make_advice(advice_type, advice_level, value, message) if {
	contains(input.name, "test")
	advice_type := "contains_test"
	advice_level := "violation"
	value := input.name
	message := "Name must not contain 'test'"
}

make_advice(advice_type, advice_level, value, message) := {
	"type": "advice",
	"advice_type": advice_type,
	"advice_level": advice_level,
	"value": value,
	"message": message,
}
