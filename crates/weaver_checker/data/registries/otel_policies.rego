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
deny contains custom_violation("registry_with_ref_attr", message, ctx) if {
	group := input.groups[_]
	startswith(group.id, "registry.")
	attr := group.attributes[_]
	attr.ref != null
	message := sprintf("Registry cannot contain ref attribute. Found group: %s, attr: %s", [group.id, attr.ref])
	ctx := {
		"group": group.id,
		"attr": attr.ref,
	}
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

# ========= Helper functions =========

# Check if an attribute from the old group exists in the new
# group's attributes
attr_exists_in_new_group(group_id, attr_id) if {
	new_group := input.groups[_]
	new_group.id == group_id
	attr := new_group.attributes[_]
	attr.id == attr_id
}

# Build an attribute registry violation
attr_registry_violation(violation_id, group_id, attr_id) := violation if {
	violation := {
		"id": violation_id,
		"type": "semconv_attribute",
		"category": "attribute_registry",
		"group": group_id,
		"attr": attr_id,
	}
}

# Build an attribute violation
attr_violation(violation_id, group_id, attr_id) := violation if {
	violation := {
		"id": violation_id,
		"type": "semconv_attribute",
		"category": "attribute",
		"group": group_id,
		"attr": attr_id,
	}
}

# Build a schema evolution violation
schema_evolution_violation(violation_id, group_id, attr_id) := violation if {
	violation := {
		"id": violation_id,
		"type": "semconv_attribute",
		"category": "schema_evolution",
		"group": group_id,
		"attr": attr_id,
	}
}

custom_violation(id, message, ctx) := violation if {
	violation := {
		"id": id,
		"type": "custom",
		"message": message,
		"context": ctx,
	}
}