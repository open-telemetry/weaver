package comparison_after_resolution

# Collect all groups from input.groups
curr_groups := {g | g := input.groups[_]; startswith(g.id, "registry.")}

# Collect all groups from data.groups
prev_groups := {g | g := data.groups[_]; startswith(g.id, "registry.")}

# Collect all group IDs from input.groups
curr_group_ids := {g.id | g := curr_groups[_]}

# Collect all group IDs from data.groups
prev_group_ids := {g.id | g := prev_groups[_]}

# Determine added group IDs in curr_group_ids that are not in prev_group_ids
added_group_ids := {id | id := curr_group_ids[_]; not prev_group_ids[id]}

# Determine removed group IDs in prev_group_ids that are not in curr_group_ids
removed_group_ids := {id | id := prev_group_ids[_]; not curr_group_ids[id]}

# Detect all added groups
deny[empty_violation()] {
    print("This group ", added_group_ids[_]," was added")
    false
}

# Detect all removed groups
deny[empty_violation()] {
    print("This group ", removed_group_ids[_], " was removed")
    false
}

empty_violation() = violation {
    violation := {
        "id": "",
        "type": "all",
        "category": "info",
        "group": "",
        "attr": "",
    }
}