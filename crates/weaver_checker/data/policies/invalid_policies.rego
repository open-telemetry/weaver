package before_resolution

import rego.v1

# Conventions for OTel:
# - `data` holds the current released semconv, which is known to be valid.
# - `input` holds the new candidate semconv version, whose validity is unknown.
#
# Note: `data` and `input` are predefined variables in Rego.

# ========= Violation rules applied on unresolved semconv files =========

# A registry `attribute_group` containing at least one `ref` attribute is
# considered invalid.
deny contains attr_registry_violation("registry_with_ref_attr", group.id, attr.ref) if {
	group := input.groups[_]
	startswith(group.id, "registry.")
	attr := group.attributes[_]
	attr.ref != null
}

# An attribute whose stability is not `deprecated` but has the deprecated field
# set to true is invalid.
deny contains attr_violation("attr_stability_deprecated", group.id, attr.id) if {
	group := input.groups[_]
	attr := group.attributes[_]
	attr.stability != "deprecated"
	attr.deprecated
}

# An attribute cannot be removed from a group that has already been released.
deny contains schema_evolution_violation("attr_removed", old_group.id, old_attr.id) if {
	old_group := data.groups[_]
	old_attr := old_group.attributes[_]
	not attr_exists_in_new_group(old_group.id, old_attr.id)
}
