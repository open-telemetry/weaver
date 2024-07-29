package after_resolution

import rego.v1

# Pre-compute const names and namespaces
const_names := {name: to_const_name(name) |
    some g in input.groups
    some attr in g.attributes
    name := attr.name
}

namespaces := {name: concat("", [name, "."]) |
    some g in input.groups
    some attr in g.attributes
    name := attr.name
}

deny contains violation if {
    some name, const_name in const_names
    not excluded_const_collisions[name]
    collisions := [other_name |
        some other_name, other_const in const_names
        other_name != name
        other_const == const_name
        not excluded_const_collisions[other_name]
    ]
    count(collisions) > 0
    violation := attr_registry_collision(
        "Attribute '%s' has the same constant name '%s' as '%s'.",
        [name, const_name, concat(", ", sort(collisions))],
        name
    )
}

deny contains violation if {
    some name, namespace in namespaces
    not excluded_namespace_collisions[name]
    collisions := [other_name |
        some other_name, other_namespace in namespaces
        startswith(other_name, namespace)
        other_name != name
        #not excluded_namespace_collisions[other_name]
    ]
    count(collisions) > 0
    violation := attr_registry_collision(
        "Attribute '%s' name is used as a namespace in the following attributes '%s'.",
        [name, concat(", ", sort(collisions))],
        name
    )
}

attr_registry_collision(description, args, attr_name) := {
    "id": sprintf(description, args),
    "type": "semconv_attribute",
    "category": "naming_collision",
    "attr": attr_name,
    "group": ""
}

to_const_name(name) := replace(name, ".", "_")

# TODO - we'll need to specify how collision resolution happens in the schema -
# see phase 2 in https://github.com/open-telemetry/semantic-conventions/issues/1118#issuecomment-2173803006
# For now just allow current collisions.
#excluded_const_collisions := {"messaging.client_id"}
excluded_const_collisions := {}
#excluded_namespace_collisions := {"messaging.operation", "db.operation", "deployment.environment"}
excluded_namespace_collisions := {}