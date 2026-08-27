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
pub fn v2_primitive_or_array_type_to_v1(
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
pub fn v2_template_type_to_v1(t: V2TemplateTypeSpec) -> V1TemplateTypeSpec {
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
pub fn v2_value_to_v1(v: V2ValueSpec) -> V1ValueSpec {
    match v {
        V2ValueSpec::Int(i) => V1ValueSpec::Int(i),
        V2ValueSpec::Double(d) => V1ValueSpec::Double(d),
        V2ValueSpec::String(s) => V1ValueSpec::String(s),
        V2ValueSpec::Bool(b) => V1ValueSpec::Bool(b),
    }
}

/// Converts a V2 enum entry to V1.
#[must_use]
pub fn v2_enum_entry_to_v1(e: V2EnumEntriesSpec) -> V1EnumEntriesSpec {
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
pub fn v2_basic_requirement_level_to_v1(
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
pub fn v2_attribute_to_v1(attr: AttributeDef) -> V1AttributeSpec {
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
pub fn v2_attribute_ref_to_v1(attr_ref: AttributeRef) -> V1AttributeSpec {
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
pub fn v2_attribute_ref_to_v1_with_role(
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
pub fn v2_span_attribute_ref_to_v1(attr_ref: SpanAttributeRef) -> V1AttributeSpec {
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
pub fn split_attributes_and_groups_to_v1(
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
pub fn split_span_attributes_and_groups_to_v1(
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
pub fn v2_metric_to_v1(metric: Metric) -> V1GroupSpec {
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
pub fn v2_metric_refinement_to_v1(r: MetricRefinement) -> V1GroupSpec {
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
pub fn v2_span_to_v1(span: Span) -> V1GroupSpec {
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
pub fn v2_span_refinement_to_v1(r: SpanRefinement) -> V1GroupSpec {
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
pub fn v2_event_to_v1(event: Event) -> V1GroupSpec {
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
pub fn v2_event_refinement_to_v1(r: EventRefinement) -> V1GroupSpec {
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
pub fn v2_entity_to_v1(entity: Entity) -> V1GroupSpec {
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
pub fn v2_entity_refinement_to_v1(r: EntityRefinement) -> V1GroupSpec {
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
pub fn v2_attribute_group_to_v1(ag: AttributeGroup) -> V1GroupSpec {
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
pub fn v2_imports_to_v1(imports: Option<V2Imports>) -> Option<V1Imports> {
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
pub fn v1_template_type_to_v2(t: V1TemplateTypeSpec) -> V2TemplateTypeSpec {
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
pub fn v1_value_to_v2(v: V1ValueSpec) -> V2ValueSpec {
    match v {
        V1ValueSpec::Int(i) => V2ValueSpec::Int(i),
        V1ValueSpec::Double(d) => V2ValueSpec::Double(d),
        V1ValueSpec::String(s) => V2ValueSpec::String(s),
        V1ValueSpec::Bool(b) => V2ValueSpec::Bool(b),
    }
}

/// Converts a V1 enum entry to V2.
#[must_use]
pub fn v1_enum_entry_to_v2(e: V1EnumEntriesSpec) -> V2EnumEntriesSpec {
    V2EnumEntriesSpec {
        id: e.id,
        value: v1_value_to_v2(e.value),
        brief: e.brief,
        note: e.note,
        stability: e.stability,
        deprecated: e.deprecated,
        annotations: Default::default(),
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
pub fn v1_basic_requirement_level_to_v2(
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

/// Converts V1 attribute group visibility to V2.
#[must_use]
pub fn v1_attribute_group_visibility_to_v2(
    v: V1VisibilitySpec,
) -> crate::v2::attribute_group::AttributeGroupVisibilitySpec {
    match v {
        V1VisibilitySpec::Public => {
            crate::v2::attribute_group::AttributeGroupVisibilitySpec::Public
        }
        V1VisibilitySpec::Internal => {
            crate::v2::attribute_group::AttributeGroupVisibilitySpec::Internal
        }
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

    #[test]
    fn test_metric_translation() {
        let yaml = r#"name: my_metric
brief: Test metric
stability: stable
instrument: histogram
unit: s
requirement_level: opt_in
"#;
        let metric = serde_yaml::from_str::<Metric>(yaml).expect("Failed to parse YAML string");
        let v1_group = v2_metric_to_v1(metric);
        assert_eq!(v1_group.id, "metric.my_metric");
        assert_eq!(v1_group.r#type, V1GroupType::Metric);
        assert_eq!(v1_group.metric_name, Some("my_metric".to_owned()));
        assert_eq!(v1_group.instrument, Some(V1InstrumentSpec::Histogram));
        assert_eq!(v1_group.unit, Some("s".to_owned()));
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
}
