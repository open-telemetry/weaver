package after_resolution

import rego.v1

# Reports the entity that every association leaf of every metric resolves to.
# The brief of the entity says which registry holds the definition, so a test can
# see what the `semconv` library found.
deny contains resolved_entity(ref, entity) if {
	some metric in input.registry.metrics
	some ref in data.semconv.entity_refs(metric.entity_associations)
	entity := data.semconv.lookup_entity(ref)
}

resolved_entity(ref, entity) := {
	"id": "resolved_entity",
	"level": "information",
	"message": sprintf("%s -> %s", [ref.type, entity.brief]),
}
