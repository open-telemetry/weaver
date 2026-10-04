package live_check_advice

import rego.v1

# Shows a policy reading the entity definitions that the jq preprocessor derives
# from the registry. The entity a signal is associated with may be defined in a
# dependency, so its definition is not in the input: it comes from
# `data.entities`, keyed by the schema url of the registry that defines it and
# then by entity type or refinement id. An association leaf holds that same
# pair, so the definition is read directly and never searched for by name.
#
# The definition is reference data, and the sample is what is checked. Here an
# entity annotates the prefix its identity values use, and the resource of the
# sample is checked against it.
#
# The dependency defines the entity, and nothing imports it:
#
#     entities:
#       - type: host
#         requirement_level: recommended
#         stability: stable
#         brief: A host.
#         annotations:
#           id_prefix: host-
#         identity:
#           - ref: host.name
#
# The registry under check defines the signal, and associates it with that entity:
#
#     events:
#       - name: thing.happened
#         requirement_level: recommended
#         stability: stable
#         brief: Something happened.
#         entity_associations:
#           - host

deny contains make_advice(advice_type, advice_level, advice_context, message) if {
	some assoc in input.registry_group.entity_associations

	# A leaf omits its provenance when this registry defines the entity.
	source := object.get(assoc, ["provenance", "source"], data.schema_url)
	entity := data.entities[source][assoc.type]
	prefix := entity.annotations.id_prefix
	some identity in entity.identity
	some attr in input.resource.attributes
	attr.name == identity.key
	not startswith(attr.value, prefix)
	advice_type := "unexpected_entity_id_prefix"
	advice_level := "improvement"
	advice_context := {
		"entity_type": assoc.type,
		"schema_url": source,
		"attribute_key": attr.name,
		"expected": prefix,
	}
	message := sprintf(
		"Value '%s' of '%s' does not start with '%s', the prefix that entity '%s' of '%s' declares",
		[attr.value, attr.name, prefix, assoc.type, source],
	)
}

make_advice(advice_type, advice_level, advice_context, message) := {
	"type": "advice",
	"advice_type": advice_type,
	"advice_level": advice_level,
	"advice_context": advice_context,
	"message": message,
}
