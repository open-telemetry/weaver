// SPDX-License-Identifier: Apache-2.0

//! Conversions between V1 and V2 semantic convention specifications.

use crate::v1::{
    attribute::{
        AttributeRole as V1AttributeRole, AttributeSpec as V1AttributeSpec,
        AttributeType as V1AttributeType, BasicRequirementLevelSpec as V1BasicRequirementLevelSpec,
        EnumEntriesSpec as V1EnumEntriesSpec, Examples as V1Examples,
        PrimitiveOrArrayTypeSpec as V1PrimitiveOrArrayTypeSpec,
        RequirementLevel as V1RequirementLevel, TemplateTypeSpec as V1TemplateTypeSpec,
        ValueSpec as V1ValueSpec,
    },
    group::{
        AttributeGroupVisibilitySpec as V1VisibilitySpec, GroupSpec as V1GroupSpec,
        GroupType as V1GroupType, GroupWildcard as V1GroupWildcard,
        InstrumentSpec as V1InstrumentSpec, SpanKindSpec as V1SpanKindSpec, SpanName as V1SpanName,
    },
    semconv::{Imports as V1Imports, SemConvSpecV1},
};
use crate::v2::{
    attribute::{
        AttributeDef, AttributeOrGroupRef, AttributeRef, AttributeType as V2AttributeType,
        BasicRequirementLevelSpec as V2BasicRequirementLevelSpec,
        EnumEntriesSpec as V2EnumEntriesSpec, Examples as V2Examples,
        PrimitiveOrArrayTypeSpec as V2PrimitiveOrArrayTypeSpec,
        RequirementLevel as V2RequirementLevel, TemplateTypeSpec as V2TemplateTypeSpec,
        ValueSpec as V2ValueSpec,
    },
    attribute_group::AttributeGroup,
    entity::{Entity, EntityRefinement},
    event::{Event, EventRefinement},
    metric::{InstrumentSpec as V2InstrumentSpec, Metric, MetricRefinement},
    span::{
        Span, SpanAttributeOrGroupRef, SpanAttributeRef, SpanKindSpec as V2SpanKindSpec,
        SpanRefinement,
    },
    Imports as V2Imports, SemConvSpecV2,
};

/// Converts a V2 primitive or array type to V1.
#[must_use]
pub(crate) fn v2_primitive_or_array_type_to_v1(
    t: V2PrimitiveOrArrayTypeSpec,
) -> V1PrimitiveOrArrayTypeSpec {
    match t {
        V2PrimitiveOrArrayTypeSpec::Boolean => V1PrimitiveOrArrayTypeSpec::Boolean,
        V2PrimitiveOrArrayTypeSpec::Int => V1PrimitiveOrArrayTypeSpec::Int,
        V2PrimitiveOrArrayTypeSpec::Double => V1PrimitiveOrArrayTypeSpec::Double,
        V2PrimitiveOrArrayTypeSpec::String => V1PrimitiveOrArrayTypeSpec::String,
        V2PrimitiveOrArrayTypeSpec::Any => V1PrimitiveOrArrayTypeSpec::Any,
        V2PrimitiveOrArrayTypeSpec::Strings => V1PrimitiveOrArrayTypeSpec::Strings,
        V2PrimitiveOrArrayTypeSpec::Ints => V1PrimitiveOrArrayTypeSpec::Ints,
        V2PrimitiveOrArrayTypeSpec::Doubles => V1PrimitiveOrArrayTypeSpec::Doubles,
        V2PrimitiveOrArrayTypeSpec::Booleans => V1PrimitiveOrArrayTypeSpec::Booleans,
    }
}

/// Converts a V2 template type to V1.
#[must_use]
pub(crate) fn v2_template_type_to_v1(t: V2TemplateTypeSpec) -> V1TemplateTypeSpec {
    match t {
        V2TemplateTypeSpec::Boolean => V1TemplateTypeSpec::Boolean,
        V2TemplateTypeSpec::Int => V1TemplateTypeSpec::Int,
        V2TemplateTypeSpec::Double => V1TemplateTypeSpec::Double,
        V2TemplateTypeSpec::String => V1TemplateTypeSpec::String,
        V2TemplateTypeSpec::Any => V1TemplateTypeSpec::Any,
        V2TemplateTypeSpec::Strings => V1TemplateTypeSpec::Strings,
        V2TemplateTypeSpec::Ints => V1TemplateTypeSpec::Ints,
        V2TemplateTypeSpec::Doubles => V1TemplateTypeSpec::Doubles,
        V2TemplateTypeSpec::Booleans => V1TemplateTypeSpec::Booleans,
    }
}

/// Converts a V2 value to V1.
#[must_use]
pub(crate) fn v2_value_to_v1(v: V2ValueSpec) -> V1ValueSpec {
    match v {
        V2ValueSpec::Int(i) => V1ValueSpec::Int(i),
        V2ValueSpec::Double(d) => V1ValueSpec::Double(d),
        V2ValueSpec::String(s) => V1ValueSpec::String(s),
        V2ValueSpec::Bool(b) => V1ValueSpec::Bool(b),
    }
}

/// Converts a V2 enum entry to V1.
#[must_use]
pub(crate) fn v2_enum_entry_to_v1(e: V2EnumEntriesSpec) -> V1EnumEntriesSpec {
    V1EnumEntriesSpec {
        id: e.id,
        value: v2_value_to_v1(e.value),
        brief: e.brief,
        note: e.note,
        stability: e.stability,
        deprecated: e.deprecated,
        annotations: e.annotations,
    }
}

/// Converts a V2 attribute type to V1.
#[must_use]
pub fn v2_attribute_type_to_v1(t: V2AttributeType) -> V1AttributeType {
    match t {
        V2AttributeType::PrimitiveOrArray(p) => {
            V1AttributeType::PrimitiveOrArray(v2_primitive_or_array_type_to_v1(p))
        }
        V2AttributeType::Template(tmpl) => V1AttributeType::Template(v2_template_type_to_v1(tmpl)),
        V2AttributeType::Enum { members } => V1AttributeType::Enum {
            members: members.into_iter().map(v2_enum_entry_to_v1).collect(),
        },
    }
}

/// Converts V2 examples to V1.
#[must_use]
pub fn v2_examples_to_v1(e: V2Examples) -> V1Examples {
    match e {
        V2Examples::Bool(b) => V1Examples::Bool(b),
        V2Examples::Int(i) => V1Examples::Int(i),
        V2Examples::Double(d) => V1Examples::Double(d),
        V2Examples::String(s) => V1Examples::String(s),
        V2Examples::Any(a) => V1Examples::Any(v2_value_to_v1(a)),
        V2Examples::Ints(ints) => V1Examples::Ints(ints),
        V2Examples::Doubles(d) => V1Examples::Doubles(d),
        V2Examples::Bools(b) => V1Examples::Bools(b),
        V2Examples::Strings(s) => V1Examples::Strings(s),
        V2Examples::Anys(anys) => V1Examples::Anys(anys.into_iter().map(v2_value_to_v1).collect()),
        V2Examples::ListOfInts(l) => V1Examples::ListOfInts(l),
        V2Examples::ListOfDoubles(l) => V1Examples::ListOfDoubles(l),
        V2Examples::ListOfBools(l) => V1Examples::ListOfBools(l),
        V2Examples::ListOfStrings(l) => V1Examples::ListOfStrings(l),
    }
}

/// Converts V2 basic requirement level to V1.
#[must_use]
pub(crate) fn v2_basic_requirement_level_to_v1(
    b: V2BasicRequirementLevelSpec,
) -> V1BasicRequirementLevelSpec {
    match b {
        V2BasicRequirementLevelSpec::Required => V1BasicRequirementLevelSpec::Required,
        V2BasicRequirementLevelSpec::Recommended => V1BasicRequirementLevelSpec::Recommended,
        V2BasicRequirementLevelSpec::OptIn => V1BasicRequirementLevelSpec::OptIn,
    }
}

/// Converts V2 requirement level to V1.
#[must_use]
pub fn v2_requirement_level_to_v1(r: V2RequirementLevel) -> V1RequirementLevel {
    match r {
        V2RequirementLevel::Basic(b) => {
            V1RequirementLevel::Basic(v2_basic_requirement_level_to_v1(b))
        }
        V2RequirementLevel::ConditionallyRequired { text } => {
            V1RequirementLevel::ConditionallyRequired { text }
        }
        V2RequirementLevel::Recommended { text } => V1RequirementLevel::Recommended { text },
        V2RequirementLevel::OptIn { text } => V1RequirementLevel::OptIn { text },
    }
}

/// Converts a V2 attribute definition into a V1 AttributeSpec.
#[must_use]
pub(crate) fn v2_attribute_to_v1(attr: AttributeDef) -> V1AttributeSpec {
    V1AttributeSpec::Id {
        id: attr.key,
        r#type: v2_attribute_type_to_v1(attr.r#type),
        brief: Some(attr.common.brief),
        examples: attr.examples.map(v2_examples_to_v1),
        tag: None,
        requirement_level: Default::default(),
        sampling_relevant: None,
        note: attr.common.note,
        stability: Some(attr.common.stability),
        deprecated: attr.common.deprecated,
        annotations: if attr.common.annotations.is_empty() {
            None
        } else {
            Some(attr.common.annotations)
        },
        role: None,
    }
}

/// Converts a V2 attribute ref into a V1 AttributeSpec.
#[must_use]
pub(crate) fn v2_attribute_ref_to_v1(attr_ref: AttributeRef) -> V1AttributeSpec {
    V1AttributeSpec::Ref {
        r#ref: attr_ref.r#ref,
        brief: attr_ref.brief,
        examples: attr_ref.examples.map(v2_examples_to_v1),
        tag: None,
        requirement_level: attr_ref.requirement_level.map(v2_requirement_level_to_v1),
        sampling_relevant: None,
        note: attr_ref.note,
        stability: None,
        deprecated: None,
        prefix: false,
        annotations: if attr_ref.annotations.is_empty() {
            None
        } else {
            Some(attr_ref.annotations)
        },
        role: None,
    }
}

/// Converts a V2 attribute ref with a specific role into a V1 AttributeSpec.
#[must_use]
pub(crate) fn v2_attribute_ref_to_v1_with_role(
    attr_ref: AttributeRef,
    role: V1AttributeRole,
) -> V1AttributeSpec {
    V1AttributeSpec::Ref {
        r#ref: attr_ref.r#ref,
        brief: attr_ref.brief,
        examples: attr_ref.examples.map(v2_examples_to_v1),
        tag: None,
        requirement_level: attr_ref.requirement_level.map(v2_requirement_level_to_v1),
        sampling_relevant: None,
        note: attr_ref.note,
        stability: None,
        deprecated: None,
        prefix: false,
        annotations: if attr_ref.annotations.is_empty() {
            None
        } else {
            Some(attr_ref.annotations)
        },
        role: Some(role),
    }
}

/// Converts a V2 span attribute ref into a V1 AttributeSpec.
#[must_use]
pub(crate) fn v2_span_attribute_ref_to_v1(attr_ref: SpanAttributeRef) -> V1AttributeSpec {
    V1AttributeSpec::Ref {
        r#ref: attr_ref.base.r#ref,
        brief: attr_ref.base.brief,
        examples: attr_ref.base.examples.map(v2_examples_to_v1),
        tag: None,
        requirement_level: attr_ref
            .base
            .requirement_level
            .map(v2_requirement_level_to_v1),
        sampling_relevant: attr_ref.sampling_relevant,
        note: attr_ref.base.note,
        stability: None,
        deprecated: None,
        prefix: false,
        annotations: if attr_ref.base.annotations.is_empty() {
            None
        } else {
            Some(attr_ref.base.annotations)
        },
        role: None,
    }
}

/// Helper function to split a vector of AttributeOrGroupRef into separate vectors
/// of V1 AttributeSpec and group reference strings.
#[must_use]
pub(crate) fn split_attributes_and_groups_to_v1(
    attributes_and_groups: Vec<AttributeOrGroupRef>,
) -> (Vec<V1AttributeSpec>, Vec<String>) {
    let mut attributes = Vec::new();
    let mut groups = Vec::new();

    for item in attributes_and_groups {
        match item {
            AttributeOrGroupRef::Attribute(attr_ref) => {
                attributes.push(v2_attribute_ref_to_v1(attr_ref));
            }
            AttributeOrGroupRef::Group(group_ref) => groups.push(group_ref.ref_group.into_v1()),
        }
    }

    (attributes, groups)
}

/// Helper function to split a vector of SpanAttributeOrGroupRef into separate vectors
/// of V1 AttributeSpec and group reference strings.
#[must_use]
pub(crate) fn split_span_attributes_and_groups_to_v1(
    attributes: Vec<SpanAttributeOrGroupRef>,
) -> (Vec<V1AttributeSpec>, Vec<String>) {
    let mut attribute_refs = Vec::new();
    let mut groups = Vec::new();

    for item in attributes {
        match item {
            SpanAttributeOrGroupRef::Attribute(attr_ref) => {
                attribute_refs.push(v2_span_attribute_ref_to_v1(attr_ref));
            }
            SpanAttributeOrGroupRef::Group(group_ref) => {
                groups.push(group_ref.ref_group);
            }
        }
    }

    (attribute_refs, groups)
}

/// Converts a V2 instrument to V1.
#[must_use]
pub fn v2_instrument_to_v1(i: V2InstrumentSpec) -> V1InstrumentSpec {
    match i {
        V2InstrumentSpec::Counter => V1InstrumentSpec::Counter,
        V2InstrumentSpec::Gauge => V1InstrumentSpec::Gauge,
        V2InstrumentSpec::Histogram => V1InstrumentSpec::Histogram,
        V2InstrumentSpec::UpDownCounter => V1InstrumentSpec::UpDownCounter,
    }
}

/// Converts a V2 span kind to V1.
#[must_use]
pub fn v2_span_kind_to_v1(k: V2SpanKindSpec) -> V1SpanKindSpec {
    match k {
        V2SpanKindSpec::Internal => V1SpanKindSpec::Internal,
        V2SpanKindSpec::Server => V1SpanKindSpec::Server,
        V2SpanKindSpec::Client => V1SpanKindSpec::Client,
        V2SpanKindSpec::Producer => V1SpanKindSpec::Producer,
        V2SpanKindSpec::Consumer => V1SpanKindSpec::Consumer,
    }
}

/// Converts a V2 span name to V1.
#[must_use]
pub fn v2_span_name_to_v1(s: crate::v2::span::SpanName) -> V1SpanName {
    V1SpanName { note: s.note }
}

/// Converts a V2 metric into a V1 GroupSpec.
#[must_use]
pub(crate) fn v2_metric_to_v1(metric: Metric) -> V1GroupSpec {
    let (attribute_refs, include_groups) = split_attributes_and_groups_to_v1(metric.attributes);
    V1GroupSpec {
        id: format!("metric.{}", &metric.name),
        r#type: V1GroupType::Metric,
        brief: metric.common.brief,
        note: metric.common.note,
        prefix: Default::default(),
        extends: None,
        include_groups,
        stability: Some(metric.common.stability),
        deprecated: metric.common.deprecated,
        attributes: attribute_refs,
        span_kind: None,
        events: Default::default(),
        metric_name: Some(metric.name.into_v1()),
        instrument: Some(v2_instrument_to_v1(metric.instrument)),
        unit: Some(metric.unit),
        name: None,
        display_name: None,
        body: None,
        annotations: if metric.common.annotations.is_empty() {
            None
        } else {
            Some(metric.common.annotations)
        },
        entity_associations: metric.entity_associations,
        visibility: None,
        is_v2: true,
        span_name: None,
        requirement_level: metric.requirement_level,
    }
}

/// Converts a V2 metric refinement into a V1 GroupSpec.
#[must_use]
pub(crate) fn v2_metric_refinement_to_v1(r: MetricRefinement) -> V1GroupSpec {
    let (attribute_refs, include_groups) = split_attributes_and_groups_to_v1(r.attributes);
    V1GroupSpec {
        id: r.id.to_string(),
        r#type: V1GroupType::Metric,
        brief: r.brief.unwrap_or_default(),
        note: r.note.unwrap_or_default(),
        prefix: Default::default(),
        extends: Some(format!("metric.{}", &r.r#ref)),
        include_groups,
        stability: r.stability,
        deprecated: r.deprecated,
        attributes: attribute_refs,
        span_kind: None,
        events: Default::default(),
        metric_name: None,
        instrument: None,
        unit: None,
        name: None,
        display_name: None,
        body: None,
        annotations: if r.annotations.is_empty() {
            None
        } else {
            Some(r.annotations)
        },
        entity_associations: r.entity_associations,
        visibility: None,
        is_v2: true,
        span_name: None,
        requirement_level: None,
    }
}

/// Converts a V2 span into a V1 GroupSpec.
#[must_use]
pub(crate) fn v2_span_to_v1(span: Span) -> V1GroupSpec {
    let (attribute_refs, include_groups) = split_span_attributes_and_groups_to_v1(span.attributes);
    V1GroupSpec {
        id: format!("span.{}", &span.r#type),
        r#type: V1GroupType::Span,
        brief: span.common.brief,
        note: span.common.note,
        prefix: Default::default(),
        extends: None,
        include_groups,
        stability: Some(span.common.stability),
        deprecated: span.common.deprecated,
        attributes: attribute_refs,
        span_kind: Some(v2_span_kind_to_v1(span.kind)),
        events: vec![],
        metric_name: None,
        instrument: None,
        unit: None,
        name: Some(format!("{}", &span.r#type)),
        display_name: None,
        body: None,
        annotations: if span.common.annotations.is_empty() {
            None
        } else {
            Some(span.common.annotations)
        },
        entity_associations: span.entity_associations,
        visibility: None,
        is_v2: true,
        span_name: Some(V1SpanName {
            note: span.name.note,
        }),
        requirement_level: span.requirement_level,
    }
}

/// Converts a V2 span refinement into a V1 GroupSpec.
#[must_use]
pub(crate) fn v2_span_refinement_to_v1(r: SpanRefinement) -> V1GroupSpec {
    let (attribute_refs, include_groups) = split_span_attributes_and_groups_to_v1(r.attributes);
    V1GroupSpec {
        id: r.id.to_string(),
        r#type: V1GroupType::Span,
        brief: r.brief.unwrap_or_default(),
        note: r.note.unwrap_or_default(),
        prefix: Default::default(),
        extends: Some(format!("span.{}", &r.r#ref)),
        include_groups,
        stability: r.stability,
        deprecated: r.deprecated,
        attributes: attribute_refs,
        span_kind: None,
        events: vec![],
        metric_name: None,
        instrument: None,
        unit: None,
        name: Some(format!("{}", &r.id)),
        display_name: None,
        body: None,
        annotations: if r.annotations.is_empty() {
            None
        } else {
            Some(r.annotations)
        },
        entity_associations: r.entity_associations,
        visibility: None,
        is_v2: true,
        span_name: r.name.map(|n| V1SpanName { note: n.note }),
        requirement_level: None,
    }
}

/// Converts a V2 event into a V1 GroupSpec.
#[must_use]
pub(crate) fn v2_event_to_v1(event: Event) -> V1GroupSpec {
    let (attribute_refs, include_groups) = split_attributes_and_groups_to_v1(event.attributes);
    V1GroupSpec {
        id: format!("event.{}", &event.name),
        r#type: V1GroupType::Event,
        brief: event.common.brief,
        note: event.common.note,
        prefix: Default::default(),
        extends: None,
        include_groups,
        stability: Some(event.common.stability),
        deprecated: event.common.deprecated,
        attributes: attribute_refs,
        span_kind: None,
        events: Default::default(),
        metric_name: None,
        instrument: None,
        unit: None,
        name: Some(event.name.into_v1()),
        display_name: None,
        body: None,
        annotations: if event.common.annotations.is_empty() {
            None
        } else {
            Some(event.common.annotations)
        },
        entity_associations: event.entity_associations,
        visibility: None,
        is_v2: true,
        span_name: None,
        requirement_level: event.requirement_level,
    }
}

/// Converts a V2 event refinement into a V1 GroupSpec.
#[must_use]
pub(crate) fn v2_event_refinement_to_v1(r: EventRefinement) -> V1GroupSpec {
    let (attribute_refs, include_groups) = split_attributes_and_groups_to_v1(r.attributes);
    V1GroupSpec {
        id: r.id.to_string(),
        r#type: V1GroupType::Event,
        brief: r.brief.unwrap_or_default(),
        note: r.note.unwrap_or_default(),
        prefix: Default::default(),
        extends: Some(format!("event.{}", &r.r#ref)),
        include_groups,
        stability: r.stability,
        deprecated: r.deprecated,
        attributes: attribute_refs,
        span_kind: None,
        events: vec![],
        metric_name: None,
        instrument: None,
        unit: None,
        name: Some(r.id.into_v1()),
        display_name: None,
        body: None,
        annotations: if r.annotations.is_empty() {
            None
        } else {
            Some(r.annotations)
        },
        entity_associations: r.entity_associations,
        visibility: None,
        is_v2: true,
        span_name: None,
        requirement_level: None,
    }
}

/// Converts a V2 entity into a V1 GroupSpec.
#[must_use]
pub(crate) fn v2_entity_to_v1(entity: Entity) -> V1GroupSpec {
    let attributes = entity
        .identity
        .into_iter()
        .map(|a| v2_attribute_ref_to_v1_with_role(a, V1AttributeRole::Identifying))
        .chain(
            entity
                .description
                .into_iter()
                .map(|a| v2_attribute_ref_to_v1_with_role(a, V1AttributeRole::Descriptive)),
        )
        .collect();

    V1GroupSpec {
        id: format!("entity.{}", &entity.r#type),
        r#type: V1GroupType::Entity,
        brief: entity.common.brief,
        note: entity.common.note,
        prefix: Default::default(),
        extends: None,
        include_groups: vec![],
        stability: Some(entity.common.stability),
        deprecated: entity.common.deprecated,
        attributes,
        span_kind: None,
        events: Default::default(),
        metric_name: None,
        instrument: None,
        unit: None,
        name: Some(entity.r#type.into_v1()),
        display_name: None,
        body: None,
        annotations: if entity.common.annotations.is_empty() {
            None
        } else {
            Some(entity.common.annotations)
        },
        entity_associations: Default::default(),
        visibility: None,
        is_v2: true,
        span_name: None,
        requirement_level: entity.requirement_level,
    }
}

/// Converts a V2 entity refinement into a V1 GroupSpec.
#[must_use]
pub(crate) fn v2_entity_refinement_to_v1(r: EntityRefinement) -> V1GroupSpec {
    let attributes = r
        .identity
        .into_iter()
        .map(|a| v2_attribute_ref_to_v1_with_role(a, V1AttributeRole::Identifying))
        .chain(
            r.description
                .into_iter()
                .map(|a| v2_attribute_ref_to_v1_with_role(a, V1AttributeRole::Descriptive)),
        )
        .collect();

    V1GroupSpec {
        id: r.id.to_string(),
        r#type: V1GroupType::Entity,
        brief: r.brief.unwrap_or_default(),
        note: r.note.unwrap_or_default(),
        prefix: Default::default(),
        extends: Some(format!("entity.{}", &r.r#ref)),
        include_groups: vec![],
        stability: r.stability,
        deprecated: r.deprecated,
        attributes,
        span_kind: None,
        events: Default::default(),
        metric_name: None,
        instrument: None,
        unit: None,
        name: Some(r.id.into_v1()),
        display_name: None,
        body: None,
        annotations: if r.annotations.is_empty() {
            None
        } else {
            Some(r.annotations)
        },
        entity_associations: Default::default(),
        visibility: None,
        is_v2: true,
        span_name: None,
        requirement_level: None,
    }
}

/// Converts a V2 attribute group into a V1 GroupSpec.
#[must_use]
pub(crate) fn v2_attribute_group_to_v1(ag: AttributeGroup) -> V1GroupSpec {
    match ag {
        AttributeGroup::Internal(internal) => {
            let (attribute_refs, include_groups) =
                split_attributes_and_groups_to_v1(internal.attributes);

            V1GroupSpec {
                id: format!("{}", &internal.id),
                r#type: V1GroupType::AttributeGroup,
                brief: format!("{}", &internal.id),
                note: "".to_owned(),
                prefix: Default::default(),
                extends: None,
                include_groups,
                stability: None,
                deprecated: None,
                attributes: attribute_refs,
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                name: None,
                display_name: None,
                body: None,
                annotations: None,
                entity_associations: vec![],
                visibility: Some(V1VisibilitySpec::Internal),
                is_v2: true,
                span_name: None,
            }
        }
        AttributeGroup::Public(public) => {
            let (attributes, include_groups) = split_attributes_and_groups_to_v1(public.attributes);

            V1GroupSpec {
                id: format!("{}", public.id),
                r#type: V1GroupType::AttributeGroup,
                brief: public.common.brief,
                note: public.common.note,
                prefix: Default::default(),
                extends: None,
                include_groups,
                stability: Some(public.common.stability),
                deprecated: public.common.deprecated,
                attributes,
                span_kind: None,
                events: vec![],
                metric_name: None,
                instrument: None,
                unit: None,
                requirement_level: None,
                name: None,
                display_name: None,
                body: None,
                annotations: if public.common.annotations.is_empty() {
                    None
                } else {
                    Some(public.common.annotations)
                },
                entity_associations: vec![],
                visibility: Some(V1VisibilitySpec::Public),
                is_v2: true,
                span_name: None,
            }
        }
    }
}

/// Converts V2 imports to V1 imports.
#[must_use]
pub(crate) fn v2_imports_to_v1(imports: Option<V2Imports>) -> Option<V1Imports> {
    imports.map(|i| V1Imports {
        metrics: i
            .metrics
            .map(|v| v.into_iter().map(|w| V1GroupWildcard(w.0)).collect()),
        events: i
            .events
            .map(|v| v.into_iter().map(|w| V1GroupWildcard(w.0)).collect()),
        entities: i
            .entities
            .map(|v| v.into_iter().map(|w| V1GroupWildcard(w.0)).collect()),
        spans: i
            .spans
            .map(|v| v.into_iter().map(|w| V1GroupWildcard(w.0)).collect()),
        attribute_groups: i
            .attribute_groups
            .map(|v| v.into_iter().map(|w| V1GroupWildcard(w.0)).collect()),
    })
}

/// Converts the version 2 schema into the version 1 group spec.
pub fn v2_to_v1_spec(spec: SemConvSpecV2, file_name: &str) -> SemConvSpecV1 {
    log::debug!("Translating v2 spec into v1 spec for {file_name}");

    let mut groups = Vec::new();

    // Only create synthetic attribute group if there are attribute definitions
    if !spec.attributes.is_empty() {
        groups.push(V1GroupSpec {
            id: format!("registry.{file_name}"),
            r#type: V1GroupType::AttributeGroup,
            attributes: spec
                .attributes
                .into_iter()
                .map(v2_attribute_to_v1)
                .collect(),
            brief: "<synthetic v2>".to_owned(),
            is_v2: true,
            span_name: None,
            ..Default::default()
        });
    }

    // Add all other groups
    groups.extend(spec.entities.into_iter().map(v2_entity_to_v1));
    groups.extend(spec.events.into_iter().map(v2_event_to_v1));
    groups.extend(spec.metrics.into_iter().map(v2_metric_to_v1));
    groups.extend(spec.spans.into_iter().map(v2_span_to_v1));
    groups.extend(
        spec.attribute_groups
            .into_iter()
            .map(v2_attribute_group_to_v1),
    );

    // Add all refinements
    groups.extend(
        spec.entity_refinements
            .into_iter()
            .map(v2_entity_refinement_to_v1),
    );
    groups.extend(
        spec.event_refinements
            .into_iter()
            .map(v2_event_refinement_to_v1),
    );
    groups.extend(
        spec.metric_refinements
            .into_iter()
            .map(v2_metric_refinement_to_v1),
    );
    groups.extend(
        spec.span_refinements
            .into_iter()
            .map(v2_span_refinement_to_v1),
    );

    SemConvSpecV1::new(groups, v2_imports_to_v1(spec.imports))
}

/// Converts a V1 primitive or array type to V2.
#[must_use]
pub fn v1_primitive_or_array_type_to_v2(
    t: V1PrimitiveOrArrayTypeSpec,
) -> V2PrimitiveOrArrayTypeSpec {
    match t {
        V1PrimitiveOrArrayTypeSpec::Boolean => V2PrimitiveOrArrayTypeSpec::Boolean,
        V1PrimitiveOrArrayTypeSpec::Int => V2PrimitiveOrArrayTypeSpec::Int,
        V1PrimitiveOrArrayTypeSpec::Double => V2PrimitiveOrArrayTypeSpec::Double,
        V1PrimitiveOrArrayTypeSpec::String => V2PrimitiveOrArrayTypeSpec::String,
        V1PrimitiveOrArrayTypeSpec::Any => V2PrimitiveOrArrayTypeSpec::Any,
        V1PrimitiveOrArrayTypeSpec::Strings => V2PrimitiveOrArrayTypeSpec::Strings,
        V1PrimitiveOrArrayTypeSpec::Ints => V2PrimitiveOrArrayTypeSpec::Ints,
        V1PrimitiveOrArrayTypeSpec::Doubles => V2PrimitiveOrArrayTypeSpec::Doubles,
        V1PrimitiveOrArrayTypeSpec::Booleans => V2PrimitiveOrArrayTypeSpec::Booleans,
    }
}

/// Converts a V1 template type to V2.
#[must_use]
pub(crate) fn v1_template_type_to_v2(t: V1TemplateTypeSpec) -> V2TemplateTypeSpec {
    match t {
        V1TemplateTypeSpec::Boolean => V2TemplateTypeSpec::Boolean,
        V1TemplateTypeSpec::Int => V2TemplateTypeSpec::Int,
        V1TemplateTypeSpec::Double => V2TemplateTypeSpec::Double,
        V1TemplateTypeSpec::String => V2TemplateTypeSpec::String,
        V1TemplateTypeSpec::Any => V2TemplateTypeSpec::Any,
        V1TemplateTypeSpec::Strings => V2TemplateTypeSpec::Strings,
        V1TemplateTypeSpec::Ints => V2TemplateTypeSpec::Ints,
        V1TemplateTypeSpec::Doubles => V2TemplateTypeSpec::Doubles,
        V1TemplateTypeSpec::Booleans => V2TemplateTypeSpec::Booleans,
    }
}

/// Converts a V1 value spec to V2.
#[must_use]
pub(crate) fn v1_value_to_v2(v: V1ValueSpec) -> V2ValueSpec {
    match v {
        V1ValueSpec::Int(i) => V2ValueSpec::Int(i),
        V1ValueSpec::Double(d) => V2ValueSpec::Double(d),
        V1ValueSpec::String(s) => V2ValueSpec::String(s),
        V1ValueSpec::Bool(b) => V2ValueSpec::Bool(b),
    }
}

/// Converts a V1 enum entry to V2.
#[must_use]
pub(crate) fn v1_enum_entry_to_v2(e: V1EnumEntriesSpec) -> V2EnumEntriesSpec {
    V2EnumEntriesSpec {
        id: e.id,
        value: v1_value_to_v2(e.value),
        brief: e.brief,
        note: e.note,
        stability: e.stability,
        deprecated: e.deprecated,
        annotations: e.annotations,
    }
}

/// Converts a V1 attribute type to V2.
#[must_use]
pub fn v1_attribute_type_to_v2(t: V1AttributeType) -> V2AttributeType {
    match t {
        V1AttributeType::PrimitiveOrArray(p) => {
            V2AttributeType::PrimitiveOrArray(v1_primitive_or_array_type_to_v2(p))
        }
        V1AttributeType::Template(tmpl) => V2AttributeType::Template(v1_template_type_to_v2(tmpl)),
        V1AttributeType::Enum { members, .. } => V2AttributeType::Enum {
            members: members.into_iter().map(v1_enum_entry_to_v2).collect(),
        },
    }
}

/// Converts V1 examples to V2 examples.
#[must_use]
pub fn v1_examples_to_v2(e: V1Examples) -> V2Examples {
    match e {
        V1Examples::Bool(b) => V2Examples::Bool(b),
        V1Examples::Int(i) => V2Examples::Int(i),
        V1Examples::Double(d) => V2Examples::Double(d),
        V1Examples::String(s) => V2Examples::String(s),
        V1Examples::Any(a) => V2Examples::Any(v1_value_to_v2(a)),
        V1Examples::Ints(ints) => V2Examples::Ints(ints),
        V1Examples::Doubles(d) => V2Examples::Doubles(d),
        V1Examples::Bools(b) => V2Examples::Bools(b),
        V1Examples::Strings(s) => V2Examples::Strings(s),
        V1Examples::Anys(anys) => V2Examples::Anys(anys.into_iter().map(v1_value_to_v2).collect()),
        V1Examples::ListOfInts(l) => V2Examples::ListOfInts(l),
        V1Examples::ListOfDoubles(l) => V2Examples::ListOfDoubles(l),
        V1Examples::ListOfBools(l) => V2Examples::ListOfBools(l),
        V1Examples::ListOfStrings(l) => V2Examples::ListOfStrings(l),
    }
}

/// Converts V1 basic requirement level to V2.
#[must_use]
pub(crate) fn v1_basic_requirement_level_to_v2(
    b: V1BasicRequirementLevelSpec,
) -> V2BasicRequirementLevelSpec {
    match b {
        V1BasicRequirementLevelSpec::Required => V2BasicRequirementLevelSpec::Required,
        V1BasicRequirementLevelSpec::Recommended => V2BasicRequirementLevelSpec::Recommended,
        V1BasicRequirementLevelSpec::OptIn => V2BasicRequirementLevelSpec::OptIn,
    }
}

/// Converts V1 requirement level to V2.
#[must_use]
pub fn v1_requirement_level_to_v2(r: V1RequirementLevel) -> V2RequirementLevel {
    match r {
        V1RequirementLevel::Basic(b) => {
            V2RequirementLevel::Basic(v1_basic_requirement_level_to_v2(b))
        }
        V1RequirementLevel::ConditionallyRequired { text } => {
            V2RequirementLevel::ConditionallyRequired { text }
        }
        V1RequirementLevel::Recommended { text } => V2RequirementLevel::Recommended { text },
        V1RequirementLevel::OptIn { text } => V2RequirementLevel::OptIn { text },
    }
}

/// Converts V1 instrument to V2.
#[must_use]
pub fn v1_instrument_to_v2(i: V1InstrumentSpec) -> V2InstrumentSpec {
    match i {
        V1InstrumentSpec::Counter => V2InstrumentSpec::Counter,
        V1InstrumentSpec::Gauge => V2InstrumentSpec::Gauge,
        V1InstrumentSpec::Histogram => V2InstrumentSpec::Histogram,
        V1InstrumentSpec::UpDownCounter => V2InstrumentSpec::UpDownCounter,
    }
}

/// Converts V1 span kind to V2.
#[must_use]
pub fn v1_span_kind_to_v2(k: V1SpanKindSpec) -> V2SpanKindSpec {
    match k {
        V1SpanKindSpec::Internal => V2SpanKindSpec::Internal,
        V1SpanKindSpec::Client => V2SpanKindSpec::Client,
        V1SpanKindSpec::Server => V2SpanKindSpec::Server,
        V1SpanKindSpec::Producer => V2SpanKindSpec::Producer,
        V1SpanKindSpec::Consumer => V2SpanKindSpec::Consumer,
    }
}

/// Converts V1 span name to V2.
#[must_use]
pub fn v1_span_name_to_v2(s: V1SpanName) -> crate::v2::span::SpanName {
    crate::v2::span::SpanName { note: s.note }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deprecated::Deprecated;
    use crate::stability::Stability;
    use crate::v1::group::SpanName as V1SpanName;
    use crate::v2::attribute::GroupRef;
    use crate::v2::signal_id::SignalId;
    use crate::v2::span::{SpanGroupRef, SpanName};
    use crate::v2::{CommonFields, GroupWildcard as V2GroupWildcard};
    use crate::YamlValue;
    use std::collections::BTreeMap;

    #[test]
    fn test_primitive_or_array_type_conversions() {
        let v2_types = [
            V2PrimitiveOrArrayTypeSpec::Boolean,
            V2PrimitiveOrArrayTypeSpec::Int,
            V2PrimitiveOrArrayTypeSpec::Double,
            V2PrimitiveOrArrayTypeSpec::String,
            V2PrimitiveOrArrayTypeSpec::Any,
            V2PrimitiveOrArrayTypeSpec::Strings,
            V2PrimitiveOrArrayTypeSpec::Ints,
            V2PrimitiveOrArrayTypeSpec::Doubles,
            V2PrimitiveOrArrayTypeSpec::Booleans,
        ];

        let v1_types = [
            V1PrimitiveOrArrayTypeSpec::Boolean,
            V1PrimitiveOrArrayTypeSpec::Int,
            V1PrimitiveOrArrayTypeSpec::Double,
            V1PrimitiveOrArrayTypeSpec::String,
            V1PrimitiveOrArrayTypeSpec::Any,
            V1PrimitiveOrArrayTypeSpec::Strings,
            V1PrimitiveOrArrayTypeSpec::Ints,
            V1PrimitiveOrArrayTypeSpec::Doubles,
            V1PrimitiveOrArrayTypeSpec::Booleans,
        ];

        for (v2, v1) in v2_types.iter().zip(v1_types.iter()) {
            assert_eq!(v2_primitive_or_array_type_to_v1(v2.clone()), v1.clone());
            assert_eq!(v1_primitive_or_array_type_to_v2(v1.clone()), v2.clone());
        }
    }

    #[test]
    fn test_template_type_conversions() {
        let v2_templates = [
            V2TemplateTypeSpec::Boolean,
            V2TemplateTypeSpec::Int,
            V2TemplateTypeSpec::Double,
            V2TemplateTypeSpec::String,
            V2TemplateTypeSpec::Any,
            V2TemplateTypeSpec::Strings,
            V2TemplateTypeSpec::Ints,
            V2TemplateTypeSpec::Doubles,
            V2TemplateTypeSpec::Booleans,
        ];

        let v1_templates = [
            V1TemplateTypeSpec::Boolean,
            V1TemplateTypeSpec::Int,
            V1TemplateTypeSpec::Double,
            V1TemplateTypeSpec::String,
            V1TemplateTypeSpec::Any,
            V1TemplateTypeSpec::Strings,
            V1TemplateTypeSpec::Ints,
            V1TemplateTypeSpec::Doubles,
            V1TemplateTypeSpec::Booleans,
        ];

        for (v2, v1) in v2_templates.iter().zip(v1_templates.iter()) {
            assert_eq!(v2_template_type_to_v1(v2.clone()), v1.clone());
            assert_eq!(v1_template_type_to_v2(v1.clone()), v2.clone());
        }
    }

    #[test]
    fn test_value_conversions() {
        let test_cases = [
            (V2ValueSpec::Int(42), V1ValueSpec::Int(42)),
            (
                V2ValueSpec::Double(2.5.into()),
                V1ValueSpec::Double(2.5.into()),
            ),
            (
                V2ValueSpec::String("hello".to_owned()),
                V1ValueSpec::String("hello".to_owned()),
            ),
            (V2ValueSpec::Bool(true), V1ValueSpec::Bool(true)),
        ];

        for (v2, v1) in test_cases {
            assert_eq!(v2_value_to_v1(v2.clone()), v1.clone());
            assert_eq!(v1_value_to_v2(v1.clone()), v2.clone());
        }
    }

    #[test]
    fn test_enum_entry_conversions() {
        let mut annotations = BTreeMap::new();
        let _ = annotations.insert(
            "custom_key".to_owned(),
            YamlValue(serde_yaml::Value::String("custom_val".to_owned())),
        );

        let v2_enum_entry = V2EnumEntriesSpec {
            id: "ok".to_owned(),
            value: V2ValueSpec::String("SUCCESS".to_owned()),
            brief: Some("Success status".to_owned()),
            note: Some("Detailed status note".to_owned()),
            stability: Some(Stability::Stable),
            deprecated: Some(Deprecated::Renamed {
                renamed_to: "new_ok".to_owned(),
                note: Some("Use new_ok instead".to_owned()),
            }),
            annotations: Some(annotations.clone()),
        };

        let v1_enum_entry = v2_enum_entry_to_v1(v2_enum_entry.clone());
        assert_eq!(v1_enum_entry.id, "ok");
        assert_eq!(
            v1_enum_entry.value,
            V1ValueSpec::String("SUCCESS".to_owned())
        );
        assert_eq!(v1_enum_entry.brief, Some("Success status".to_owned()));
        assert_eq!(v1_enum_entry.note, Some("Detailed status note".to_owned()));
        assert_eq!(v1_enum_entry.stability, Some(Stability::Stable));
        assert_eq!(v1_enum_entry.annotations, Some(annotations));

        let converted_back_v2 = v1_enum_entry_to_v2(v1_enum_entry);
        assert_eq!(converted_back_v2.id, v2_enum_entry.id);
        assert_eq!(converted_back_v2.value, v2_enum_entry.value);
        assert_eq!(converted_back_v2.brief, v2_enum_entry.brief);
        assert_eq!(converted_back_v2.note, v2_enum_entry.note);
        assert_eq!(converted_back_v2.stability, v2_enum_entry.stability);
        assert_eq!(converted_back_v2.deprecated, v2_enum_entry.deprecated);
        assert_eq!(converted_back_v2.annotations, v2_enum_entry.annotations);
    }

    #[test]
    fn test_attribute_type_conversions() {
        let primitive_v2 = V2AttributeType::PrimitiveOrArray(V2PrimitiveOrArrayTypeSpec::String);
        let primitive_v1 = v2_attribute_type_to_v1(primitive_v2.clone());
        assert_eq!(
            primitive_v1,
            V1AttributeType::PrimitiveOrArray(V1PrimitiveOrArrayTypeSpec::String)
        );
        assert_eq!(v1_attribute_type_to_v2(primitive_v1), primitive_v2);

        let template_v2 = V2AttributeType::Template(V2TemplateTypeSpec::Int);
        let template_v1 = v2_attribute_type_to_v1(template_v2.clone());
        assert_eq!(
            template_v1,
            V1AttributeType::Template(V1TemplateTypeSpec::Int)
        );
        assert_eq!(v1_attribute_type_to_v2(template_v1), template_v2);

        let enum_v2 = V2AttributeType::Enum {
            members: vec![V2EnumEntriesSpec {
                id: "item1".to_owned(),
                value: V2ValueSpec::Int(1),
                brief: None,
                note: None,
                stability: None,
                deprecated: None,
                annotations: Default::default(),
            }],
        };
        let enum_v1 = v2_attribute_type_to_v1(enum_v2.clone());
        assert!(matches!(enum_v1, V1AttributeType::Enum { ref members, .. } if members.len() == 1));
        assert_eq!(v1_attribute_type_to_v2(enum_v1), enum_v2);
    }

    #[test]
    fn test_examples_conversions() {
        let examples_cases = [
            (V2Examples::Bool(true), V1Examples::Bool(true)),
            (V2Examples::Int(42), V1Examples::Int(42)),
            (
                V2Examples::Double(2.5.into()),
                V1Examples::Double(2.5.into()),
            ),
            (
                V2Examples::String("example".to_owned()),
                V1Examples::String("example".to_owned()),
            ),
            (
                V2Examples::Any(V2ValueSpec::Int(10)),
                V1Examples::Any(V1ValueSpec::Int(10)),
            ),
            (
                V2Examples::Ints(vec![1, 2, 3]),
                V1Examples::Ints(vec![1, 2, 3]),
            ),
            (
                V2Examples::Doubles(vec![1.1.into(), 2.2.into()]),
                V1Examples::Doubles(vec![1.1.into(), 2.2.into()]),
            ),
            (
                V2Examples::Bools(vec![true, false]),
                V1Examples::Bools(vec![true, false]),
            ),
            (
                V2Examples::Strings(vec!["a".to_owned(), "b".to_owned()]),
                V1Examples::Strings(vec!["a".to_owned(), "b".to_owned()]),
            ),
            (
                V2Examples::Anys(vec![
                    V2ValueSpec::String("val".to_owned()),
                    V2ValueSpec::Bool(true),
                ]),
                V1Examples::Anys(vec![
                    V1ValueSpec::String("val".to_owned()),
                    V1ValueSpec::Bool(true),
                ]),
            ),
            (
                V2Examples::ListOfInts(vec![vec![1, 2], vec![3]]),
                V1Examples::ListOfInts(vec![vec![1, 2], vec![3]]),
            ),
            (
                V2Examples::ListOfDoubles(vec![vec![1.1.into()]]),
                V1Examples::ListOfDoubles(vec![vec![1.1.into()]]),
            ),
            (
                V2Examples::ListOfBools(vec![vec![true, false]]),
                V1Examples::ListOfBools(vec![vec![true, false]]),
            ),
            (
                V2Examples::ListOfStrings(vec![vec!["s1".to_owned(), "s2".to_owned()]]),
                V1Examples::ListOfStrings(vec![vec!["s1".to_owned(), "s2".to_owned()]]),
            ),
        ];

        for (v2, v1) in examples_cases {
            assert_eq!(v2_examples_to_v1(v2.clone()), v1.clone());
            assert_eq!(v1_examples_to_v2(v1.clone()), v2.clone());
        }
    }

    #[test]
    fn test_requirement_level_conversions() {
        let req_cases = [
            (
                V2RequirementLevel::Basic(V2BasicRequirementLevelSpec::Required),
                V1RequirementLevel::Basic(V1BasicRequirementLevelSpec::Required),
            ),
            (
                V2RequirementLevel::Basic(V2BasicRequirementLevelSpec::Recommended),
                V1RequirementLevel::Basic(V1BasicRequirementLevelSpec::Recommended),
            ),
            (
                V2RequirementLevel::Basic(V2BasicRequirementLevelSpec::OptIn),
                V1RequirementLevel::Basic(V1BasicRequirementLevelSpec::OptIn),
            ),
            (
                V2RequirementLevel::ConditionallyRequired {
                    text: "when enabled".to_owned(),
                },
                V1RequirementLevel::ConditionallyRequired {
                    text: "when enabled".to_owned(),
                },
            ),
            (
                V2RequirementLevel::Recommended {
                    text: "for diagnostics".to_owned(),
                },
                V1RequirementLevel::Recommended {
                    text: "for diagnostics".to_owned(),
                },
            ),
            (
                V2RequirementLevel::OptIn {
                    text: "experimental feature".to_owned(),
                },
                V1RequirementLevel::OptIn {
                    text: "experimental feature".to_owned(),
                },
            ),
        ];

        for (v2, v1) in req_cases {
            assert_eq!(v2_requirement_level_to_v1(v2.clone()), v1.clone());
            assert_eq!(v1_requirement_level_to_v2(v1.clone()), v2.clone());
        }
    }

    #[test]
    fn test_instrument_conversions() {
        let cases = [
            (V2InstrumentSpec::Counter, V1InstrumentSpec::Counter),
            (V2InstrumentSpec::Gauge, V1InstrumentSpec::Gauge),
            (V2InstrumentSpec::Histogram, V1InstrumentSpec::Histogram),
            (
                V2InstrumentSpec::UpDownCounter,
                V1InstrumentSpec::UpDownCounter,
            ),
        ];

        for (v2, v1) in cases {
            assert_eq!(v2_instrument_to_v1(v2), v1);
            assert_eq!(v1_instrument_to_v2(v1), v2);
        }
    }

    #[test]
    fn test_span_kind_conversions() {
        let cases = [
            (V2SpanKindSpec::Internal, V1SpanKindSpec::Internal),
            (V2SpanKindSpec::Server, V1SpanKindSpec::Server),
            (V2SpanKindSpec::Client, V1SpanKindSpec::Client),
            (V2SpanKindSpec::Producer, V1SpanKindSpec::Producer),
            (V2SpanKindSpec::Consumer, V1SpanKindSpec::Consumer),
        ];

        for (v2, v1) in cases {
            assert_eq!(v2_span_kind_to_v1(v2), v1);
            assert_eq!(v1_span_kind_to_v2(v1), v2);
        }
    }

    #[test]
    fn test_span_name_conversions() {
        let v2_span_name = SpanName {
            note: "HTTP {method}".to_owned(),
        };
        let v1_span_name = v2_span_name_to_v1(v2_span_name.clone());
        assert_eq!(v1_span_name.note, "HTTP {method}");
        assert_eq!(v1_span_name_to_v2(v1_span_name).note, v2_span_name.note);
    }

    #[test]
    fn test_attribute_def_and_ref_to_v1() {
        let mut annotations = BTreeMap::new();
        let _ = annotations.insert(
            "tier".to_owned(),
            YamlValue(serde_yaml::Value::String("core".to_owned())),
        );

        let attr_def = AttributeDef {
            key: "http.status_code".to_owned(),
            r#type: V2AttributeType::PrimitiveOrArray(V2PrimitiveOrArrayTypeSpec::Int),
            examples: Some(V2Examples::Int(200)),
            common: CommonFields {
                brief: "HTTP response status code".to_owned(),
                note: "Note text".to_owned(),
                stability: Stability::Stable,
                deprecated: Some(Deprecated::Renamed {
                    renamed_to: "response.status_code".to_owned(),
                    note: Some("Use response.status_code".to_owned()),
                }),
                annotations: annotations.clone(),
            },
        };

        let v1_spec = v2_attribute_to_v1(attr_def);
        match v1_spec {
            V1AttributeSpec::Id {
                id,
                r#type,
                brief,
                examples,
                stability,
                deprecated,
                annotations: ans,
                ..
            } => {
                assert_eq!(id, "http.status_code");
                assert_eq!(
                    r#type,
                    V1AttributeType::PrimitiveOrArray(V1PrimitiveOrArrayTypeSpec::Int)
                );
                assert_eq!(brief, Some("HTTP response status code".to_owned()));
                assert_eq!(examples, Some(V1Examples::Int(200)));
                assert_eq!(stability, Some(Stability::Stable));
                assert!(deprecated.is_some());
                assert_eq!(ans, Some(annotations.clone()));
            }
            V1AttributeSpec::Ref { .. } => panic!("Expected V1AttributeSpec::Id"),
        }

        let attr_ref = AttributeRef {
            r#ref: "http.status_code".to_owned(),
            brief: Some("Override brief".to_owned()),
            examples: Some(V2Examples::Int(404)),
            requirement_level: Some(V2RequirementLevel::Basic(
                V2BasicRequirementLevelSpec::Required,
            )),
            note: Some("Override note".to_owned()),
            annotations: annotations.clone(),
        };

        let v1_ref = v2_attribute_ref_to_v1(attr_ref.clone());
        match v1_ref {
            V1AttributeSpec::Ref {
                r#ref,
                brief,
                examples,
                requirement_level,
                note,
                annotations: ans,
                role,
                ..
            } => {
                assert_eq!(r#ref, "http.status_code");
                assert_eq!(brief, Some("Override brief".to_owned()));
                assert_eq!(examples, Some(V1Examples::Int(404)));
                assert_eq!(
                    requirement_level,
                    Some(V1RequirementLevel::Basic(
                        V1BasicRequirementLevelSpec::Required
                    ))
                );
                assert_eq!(note, Some("Override note".to_owned()));
                assert_eq!(ans, Some(annotations.clone()));
                assert_eq!(role, None);
            }
            V1AttributeSpec::Id { .. } => panic!("Expected V1AttributeSpec::Ref"),
        }

        let v1_role_ref = v2_attribute_ref_to_v1_with_role(attr_ref, V1AttributeRole::Identifying);
        match v1_role_ref {
            V1AttributeSpec::Ref { role, .. } => {
                assert_eq!(role, Some(V1AttributeRole::Identifying));
            }
            V1AttributeSpec::Id { .. } => panic!("Expected V1AttributeSpec::Ref with role"),
        }
    }

    #[test]
    fn test_split_attributes_and_groups() {
        let items = vec![
            AttributeOrGroupRef::Attribute(AttributeRef {
                r#ref: "attr1".to_owned(),
                brief: None,
                examples: None,
                requirement_level: None,
                note: None,
                annotations: Default::default(),
            }),
            AttributeOrGroupRef::Group(GroupRef {
                ref_group: SignalId::from("my_group"),
            }),
        ];

        let (attrs, groups) = split_attributes_and_groups_to_v1(items);
        assert_eq!(attrs.len(), 1);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], "my_group");
    }

    #[test]
    fn test_split_span_attributes_and_groups() {
        let items = vec![
            SpanAttributeOrGroupRef::Attribute(SpanAttributeRef {
                base: AttributeRef {
                    r#ref: "attr1".to_owned(),
                    brief: None,
                    examples: None,
                    requirement_level: None,
                    note: None,
                    annotations: Default::default(),
                },
                sampling_relevant: Some(true),
            }),
            SpanAttributeOrGroupRef::Group(SpanGroupRef {
                ref_group: "span_group".to_owned(),
            }),
        ];

        let (attrs, groups) = split_span_attributes_and_groups_to_v1(items);
        assert_eq!(attrs.len(), 1);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], "span_group");
        match &attrs[0] {
            V1AttributeSpec::Ref {
                sampling_relevant, ..
            } => {
                assert_eq!(*sampling_relevant, Some(true));
            }
            V1AttributeSpec::Id { .. } => panic!("Expected V1AttributeSpec::Ref"),
        }
    }

    #[test]
    fn test_metric_translation() {
        let yaml = r#"name: my_metric
brief: Test metric
note: Metric note
stability: stable
instrument: histogram
unit: s
requirement_level: opt_in
annotations:
  custom: val
attributes:
  - ref: some_attr
  - ref_group: some_group
"#;
        let metric = serde_yaml::from_str::<Metric>(yaml).expect("Failed to parse YAML string");
        let v1_group = v2_metric_to_v1(metric);
        assert_eq!(v1_group.id, "metric.my_metric");
        assert_eq!(v1_group.r#type, V1GroupType::Metric);
        assert_eq!(v1_group.metric_name, Some("my_metric".to_owned()));
        assert_eq!(v1_group.instrument, Some(V1InstrumentSpec::Histogram));
        assert_eq!(v1_group.unit, Some("s".to_owned()));
        assert_eq!(v1_group.attributes.len(), 1);
        assert_eq!(v1_group.include_groups.len(), 1);
        assert_eq!(v1_group.include_groups[0], "some_group");
        assert!(v1_group.annotations.is_some());
        assert!(v1_group.is_v2);
    }

    #[test]
    fn test_metric_refinement_translation() {
        let refinement = MetricRefinement {
            id: SignalId::from("my_metric_refinement"),
            r#ref: SignalId::from("original_metric"),
            brief: Some("Refined brief".to_owned()),
            note: Some("Refined note".to_owned()),
            stability: Some(Stability::Stable),
            deprecated: None,
            attributes: vec![AttributeOrGroupRef::Attribute(AttributeRef {
                r#ref: "extra_attr".to_owned(),
                brief: None,
                examples: None,
                requirement_level: None,
                note: None,
                annotations: Default::default(),
            })],
            annotations: Default::default(),
            entity_associations: vec![],
        };

        let v1_group = v2_metric_refinement_to_v1(refinement);
        assert_eq!(v1_group.id, "my_metric_refinement");
        assert_eq!(v1_group.r#type, V1GroupType::Metric);
        assert_eq!(v1_group.extends, Some("metric.original_metric".to_owned()));
        assert_eq!(v1_group.brief, "Refined brief");
        assert_eq!(v1_group.attributes.len(), 1);
        assert!(v1_group.is_v2);
    }

    #[test]
    fn test_span_translation() {
        let span = Span {
            r#type: SignalId::from("http.client"),
            kind: V2SpanKindSpec::Client,
            name: SpanName {
                note: "HTTP {http.request.method}".to_owned(),
            },
            common: CommonFields {
                brief: "Client HTTP span".to_owned(),
                note: "Span details".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
            attributes: vec![SpanAttributeOrGroupRef::Attribute(SpanAttributeRef {
                base: AttributeRef {
                    r#ref: "http.request.method".to_owned(),
                    brief: None,
                    examples: None,
                    requirement_level: None,
                    note: None,
                    annotations: Default::default(),
                },
                sampling_relevant: Some(true),
            })],
            entity_associations: vec![],
            requirement_level: None,
        };

        let v1_group = v2_span_to_v1(span);
        assert_eq!(v1_group.id, "span.http.client");
        assert_eq!(v1_group.r#type, V1GroupType::Span);
        assert_eq!(v1_group.span_kind, Some(V1SpanKindSpec::Client));
        assert_eq!(
            v1_group.span_name,
            Some(V1SpanName {
                note: "HTTP {http.request.method}".to_owned()
            })
        );
        assert_eq!(v1_group.attributes.len(), 1);
        assert!(v1_group.is_v2);
    }

    #[test]
    fn test_span_refinement_translation() {
        let refinement = SpanRefinement {
            id: SignalId::from("http.client.refined"),
            r#ref: SignalId::from("http.client"),
            name: Some(SpanName {
                note: "Overridden name".to_owned(),
            }),
            brief: Some("Refined span brief".to_owned()),
            note: Some("Refined span note".to_owned()),
            stability: Some(Stability::Stable),
            deprecated: None,
            attributes: vec![],
            annotations: Default::default(),
            entity_associations: vec![],
        };

        let v1_group = v2_span_refinement_to_v1(refinement);
        assert_eq!(v1_group.id, "http.client.refined");
        assert_eq!(v1_group.r#type, V1GroupType::Span);
        assert_eq!(v1_group.extends, Some("span.http.client".to_owned()));
        assert_eq!(
            v1_group.span_name,
            Some(V1SpanName {
                note: "Overridden name".to_owned()
            })
        );
        assert!(v1_group.is_v2);
    }

    #[test]
    fn test_event_translation() {
        let event = Event {
            name: SignalId::from("exception"),
            common: CommonFields {
                brief: "An exception occurred".to_owned(),
                note: "Event details".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
            attributes: vec![AttributeOrGroupRef::Attribute(AttributeRef {
                r#ref: "exception.type".to_owned(),
                brief: None,
                examples: None,
                requirement_level: None,
                note: None,
                annotations: Default::default(),
            })],
            entity_associations: vec![],
            requirement_level: None,
        };

        let v1_group = v2_event_to_v1(event);
        assert_eq!(v1_group.id, "event.exception");
        assert_eq!(v1_group.r#type, V1GroupType::Event);
        assert_eq!(v1_group.name, Some("exception".to_owned()));
        assert_eq!(v1_group.attributes.len(), 1);
        assert!(v1_group.is_v2);
    }

    #[test]
    fn test_event_refinement_translation() {
        let refinement = EventRefinement {
            id: SignalId::from("exception.refined"),
            r#ref: SignalId::from("exception"),
            brief: Some("Refined exception".to_owned()),
            note: None,
            stability: Some(Stability::Stable),
            deprecated: None,
            attributes: vec![],
            annotations: Default::default(),
            entity_associations: vec![],
        };

        let v1_group = v2_event_refinement_to_v1(refinement);
        assert_eq!(v1_group.id, "exception.refined");
        assert_eq!(v1_group.r#type, V1GroupType::Event);
        assert_eq!(v1_group.extends, Some("event.exception".to_owned()));
        assert!(v1_group.is_v2);
    }

    #[test]
    fn test_entity_translation() {
        let yaml = r#"type: my_entity
identity:
  - ref: some_attr
description:
  - ref: some_other_attr
brief: Test entity
stability: stable
"#;
        let entity = serde_yaml::from_str::<Entity>(yaml).expect("Failed to parse YAML string");
        let v1_group = v2_entity_to_v1(entity);
        assert_eq!(v1_group.id, "entity.my_entity");
        assert_eq!(v1_group.r#type, V1GroupType::Entity);
        assert_eq!(v1_group.attributes.len(), 2);
        assert!(v1_group.is_v2);
    }

    #[test]
    fn test_entity_refinement_translation() {
        let refinement = EntityRefinement {
            id: SignalId::from("host.refined"),
            r#ref: SignalId::from("host"),
            identity: vec![AttributeRef {
                r#ref: "host.id".to_owned(),
                brief: None,
                examples: None,
                requirement_level: None,
                note: None,
                annotations: Default::default(),
            }],
            description: vec![AttributeRef {
                r#ref: "host.name".to_owned(),
                brief: None,
                examples: None,
                requirement_level: None,
                note: None,
                annotations: Default::default(),
            }],
            brief: Some("Refined host".to_owned()),
            note: None,
            stability: Some(Stability::Stable),
            deprecated: None,
            annotations: Default::default(),
        };

        let v1_group = v2_entity_refinement_to_v1(refinement);
        assert_eq!(v1_group.id, "host.refined");
        assert_eq!(v1_group.r#type, V1GroupType::Entity);
        assert_eq!(v1_group.extends, Some("entity.host".to_owned()));
        assert_eq!(v1_group.attributes.len(), 2);
        assert!(v1_group.is_v2);
    }

    #[test]
    fn test_attribute_group_translation() {
        let internal_ag =
            AttributeGroup::Internal(crate::v2::attribute_group::InternalAttributeGroup {
                id: SignalId::from("internal.group"),
                attributes: vec![AttributeOrGroupRef::Attribute(AttributeRef {
                    r#ref: "some.attr".to_owned(),
                    brief: None,
                    examples: None,
                    requirement_level: None,
                    note: None,
                    annotations: Default::default(),
                })],
            });

        let v1_internal = v2_attribute_group_to_v1(internal_ag);
        assert_eq!(v1_internal.id, "internal.group");
        assert_eq!(v1_internal.r#type, V1GroupType::AttributeGroup);
        assert_eq!(v1_internal.visibility, Some(V1VisibilitySpec::Internal));
        assert_eq!(v1_internal.attributes.len(), 1);

        let public_ag = AttributeGroup::Public(crate::v2::attribute_group::PublicAttributeGroup {
            id: SignalId::from("public.group"),
            common: CommonFields {
                brief: "Public attribute group".to_owned(),
                note: "Group notes".to_owned(),
                stability: Stability::Stable,
                deprecated: None,
                annotations: Default::default(),
            },
            attributes: vec![],
        });

        let v1_public = v2_attribute_group_to_v1(public_ag);
        assert_eq!(v1_public.id, "public.group");
        assert_eq!(v1_public.r#type, V1GroupType::AttributeGroup);
        assert_eq!(v1_public.visibility, Some(V1VisibilitySpec::Public));
        assert_eq!(v1_public.brief, "Public attribute group");
    }

    #[test]
    fn test_imports_translation() {
        let v2_imports = V2Imports {
            metrics: Some(vec![V2GroupWildcard(
                globset::Glob::new("metric.*").unwrap(),
            )]),
            events: Some(vec![V2GroupWildcard(
                globset::Glob::new("event.*").unwrap(),
            )]),
            entities: Some(vec![V2GroupWildcard(
                globset::Glob::new("entity.*").unwrap(),
            )]),
            spans: Some(vec![V2GroupWildcard(globset::Glob::new("span.*").unwrap())]),
            attribute_groups: Some(vec![V2GroupWildcard(
                globset::Glob::new("group.*").unwrap(),
            )]),
        };

        let v1_imports = v2_imports_to_v1(Some(v2_imports)).expect("Imports should be present");
        assert_eq!(v1_imports.metrics.as_ref().map(|v| v.len()), Some(1));
        assert_eq!(v1_imports.events.as_ref().map(|v| v.len()), Some(1));
        assert_eq!(v1_imports.entities.as_ref().map(|v| v.len()), Some(1));
        assert_eq!(v1_imports.spans.as_ref().map(|v| v.len()), Some(1));
        assert_eq!(
            v1_imports.attribute_groups.as_ref().map(|v| v.len()),
            Some(1)
        );
    }

    #[test]
    fn test_v2_to_v1_full_spec_translation() {
        let spec_v2 = SemConvSpecV2 {
            attributes: vec![AttributeDef {
                key: "http.method".to_owned(),
                r#type: V2AttributeType::PrimitiveOrArray(V2PrimitiveOrArrayTypeSpec::String),
                examples: Some(V2Examples::String("GET".to_owned())),
                common: CommonFields {
                    brief: "HTTP method".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: Default::default(),
                },
            }],
            metrics: vec![Metric {
                name: SignalId::from("http.server.duration"),
                instrument: V2InstrumentSpec::Histogram,
                unit: "ms".to_owned(),
                common: CommonFields {
                    brief: "Server duration".to_owned(),
                    note: "".to_owned(),
                    stability: Stability::Stable,
                    deprecated: None,
                    annotations: Default::default(),
                },
                attributes: vec![],
                entity_associations: vec![],
                requirement_level: None,
            }],
            events: vec![],
            entities: vec![],
            spans: vec![],
            attribute_groups: vec![],
            metric_refinements: vec![],
            event_refinements: vec![],
            entity_refinements: vec![],
            span_refinements: vec![],
            imports: None,
        };

        let spec_v1 = v2_to_v1_spec(spec_v2, "http");
        // Should produce a synthetic attribute group + 1 metric group
        assert_eq!(spec_v1.groups.len(), 2);
        assert_eq!(spec_v1.groups[0].id, "registry.http");
        assert_eq!(spec_v1.groups[0].brief, "<synthetic v2>");
        assert_eq!(spec_v1.groups[1].id, "metric.http.server.duration");
    }
}
