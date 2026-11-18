package live_check_advice.builtin

import rego.v1

# Builds a finding.
make_advice(advice_type, advice_level, advice_context, message) := {
	"type": "advice",
	"advice_type": advice_type,
	"advice_level": advice_level,
	"advice_context": advice_context,
	"message": message,
}

# Builds a finding that names the signal it belongs to.
make_advice_with_signal_info(advice_type, advice_level, advice_context, signal_name, signal_type, message) := {
	"type": "advice",
	"advice_type": advice_type,
	"advice_level": advice_level,
	"advice_context": advice_context,
	"signal_name": signal_name,
	"signal_type": signal_type,
	"message": message,
}
