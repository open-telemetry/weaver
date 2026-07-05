package live_check_advice

import rego.v1

# Fires for a fixed attribute name regardless of any external data.
# Proves the custom advice_policies directory is loaded and evaluated.
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
	input.sample.attribute
	input.sample.attribute.name == "custom.sentinel"
	advice_type := "custom_sentinel"
	advice_level := "violation"
	advice_context := {"attribute_key": input.sample.attribute.name}
	message := "custom sentinel policy fired"
}

# Fires when the attribute name appears in the denylist supplied via advice_data.
# Proves advice_data (data/denylist.json -> data.denylist) is loaded into OPA.
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
	input.sample.attribute
	some blocked in data.denylist.attributes
	input.sample.attribute.name == blocked
	advice_type := "custom_denylisted"
	advice_level := "violation"
	advice_context := {"attribute_key": input.sample.attribute.name}
	message := sprintf("Attribute '%s' is denylisted by custom policy", [input.sample.attribute.name])
}

make_advice(advice_type, advice_level, advice_context, message) := {
	"type": "advice",
	"advice_type": advice_type,
	"advice_level": advice_level,
	"advice_context": advice_context,
	"message": message,
}
