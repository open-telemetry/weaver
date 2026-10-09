package after_resolution

import rego.v1

# A stable signal must not be associated with an entity that is not stable.
# Stability is on the entity definition, not on the association, so the policy
# has to read the definition.
#
# `entity_refs` walks the `one_of` and `all_of` levels of the association, and
# `lookup_entity` reads the definition, whether this registry or a dependency
# defines it. Both come from the `semconv` library that weaver loads. A policy
# must call them by their full path.
deny contains unstable_entity_finding(signal_name(signal), entity) if {
	some kind in ["metrics", "events", "spans"]
	some signal in input.registry[kind]
	signal.stability == "stable"
	some ref in data.semconv.entity_refs(signal.entity_associations)
	entity := data.semconv.lookup_entity(ref)
	entity.stability != "stable"
}

# A metric or an event has a name. A span has a type.
signal_name(signal) := signal.name

signal_name(signal) := signal.type if not signal.name

unstable_entity_finding(name, entity) := {
	"id": "unstable_entity_association",
	"level": "violation",
	"signal_name": name,
	"message": sprintf("Stable signal '%s' is associated with entity '%s', which is '%s'", [name, entity.type, entity.stability]),
	"context": {"entity_type": entity.type, "entity_stability": entity.stability},
}
