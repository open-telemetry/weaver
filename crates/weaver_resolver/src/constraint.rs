// SPDX-License-Identifier: Apache-2.0

//! Functions to resolve a semantic convention constraint field.

use weaver_semconv::group::ConstraintSpec;

/// Resolve a list of semantic convention constraints.
pub fn resolve_constraints(
    constraints: &[ConstraintSpec],
) -> Vec<weaver_resolved_schema::registry::Constraint> {
    constraints.iter().map(resolve_constraint).collect()
}

/// Resolve a semantic convention constraint.
pub fn resolve_constraint(
    constraint: &ConstraintSpec,
) -> weaver_resolved_schema::registry::Constraint {
    weaver_resolved_schema::registry::Constraint {
        any_of: constraint.any_of.clone(),
        include: constraint.include.clone(),
    }
}
