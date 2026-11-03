package comparison_after_resolution
import rego.v1

deny contains {
    "id": "compatibility_entity_missing",
    "message": "Legacy dependency entity remains visible to comparison policies",
    "level": "violation",
    "signal_type": "entity",
    "signal_name": entity.type,
} if {
    some dependency in data.dependencies
    some entity in dependency.registry.entities
    entity.type == "legacy"
    count(object.get(entity, "identity", [])) == 0
    some attribute in entity.description
    attribute.key == "legacy.name"
}
