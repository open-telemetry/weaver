package advice

import rego.v1

# checks attribute name contains the word "test"
deny contains make_advice(key, advisory, value, message) if {
	contains(input.name, "test")
	key := "contains_test"
	advisory := "violation"
	value := input.name
	message := "Name must not contain 'test'"
}

make_advice(key, advisory, value, message) := {
	"type": "advice",
	"key": key,
	"advisory": advisory,
	"value": value,
	"message": message,
}
