// SPDX-License-Identifier: Apache-2.0

//! Functions to resolve a semantic convention body.

use weaver_resolved_schema::any_value::AnyValue;
use weaver_semconv::{any_value::AnyValueSpec, attribute::EnumEntriesSpec};

/// Resolve a `Body` specification into a resolved `Body`.
#[must_use]
pub fn resolve_any_value_spec(value: &AnyValueSpec) -> AnyValue {
    match value {
        AnyValueSpec::Map { fields, .. } => {
            let resolved_fields: Vec<AnyValue> = fields.iter()
                .map(resolve_any_value_spec)
                .collect();

            construct_any_value_common(value, Some(resolved_fields), None, None)
        },
        AnyValueSpec::Enum { allow_custom_values, members, .. } => {
            construct_any_value_common(value, None, Some(*allow_custom_values), Some(members.to_vec()))
        },
        _ => construct_any_value_common(value, None, None, None),
    }
}

/// Construct an AnyValue with common fields.
fn construct_any_value_common(
    value: &AnyValueSpec,
    resolved_fields: Option<Vec<AnyValue>>,
    allow_custom_values: Option<bool>,
    members: Option<Vec<EnumEntriesSpec>>) -> AnyValue {
    let common = value.common();

    AnyValue {
        name: value.id(),
        r#type: value.type_name(),
        type_display: Some(value.to_string()),
        brief: value.brief(),
        note: value.note(),
        stability: common.stability.clone(),
        examples: common.examples.clone(),
        fields: resolved_fields,
        requirement_level: common.requirement_level.clone(),
        deprecated: common.deprecated.clone(),
        allow_custom_values,
        members,
    }
}
