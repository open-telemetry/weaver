package live_check_advice

import rego.v1

# Causes: "error: use of undefined variable `attribu1te_name` is unsafe"
deny contains make_advice("foo", "violation", attribute_name, "bar") if {
	attribute_name := "foo"
	not attribu1te_name
}

make_advice(advice_type, advice_level, advice_context, message) := {
	"type": "advice",
	"advice_type": advice_type,
	"advice_level": advice_level,
	"advice_context": advice_context,
	"message": message,
}
