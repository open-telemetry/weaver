//! Span related definitions structs.

use crate::v2::attribute::Attribute;
use crate::v2::entity::EntityAssociation;
use crate::v2::provenance::Provenance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::{
    signal_requirement_level::SignalRequirementLevel,
    v2::{
        attribute::RequirementLevel,
        signal_id::SignalId,
        span::{SpanKindSpec, SpanName},
        CommonFields,
    },
};

/// The definition of a span signal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct Span {
    /// The type of the Span. This denotes the identity
    /// of the "shape" of this span, and must be unique.
    pub r#type: SignalId,
    /// Specifies the kind of the span.
    pub kind: SpanKindSpec,
    /// The name pattern for the span.
    pub name: SpanName,
    /// List of attributes that should be included on this span.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<SpanAttribute>,
    /// Which resources this span should be associated with.
    ///
    /// The list is an implicit `one_of` (telemetry must satisfy at least one entry); each entry is an
    /// entity reference or a nested `one_of`/`all_of` expression.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<EntityAssociation>,

    /// The requirement level of the span. Defaults to 'recommended' when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_level: Option<SignalRequirementLevel>,

    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
    /// The provenance of the span.
    #[serde(default)]
    #[serde(skip_serializing_if = "Provenance::is_empty")]
    pub provenance: Provenance,
}

/// A special type of reference to attributes that remembers span-specicific information.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct SpanAttribute {
    /// Base attribute definitions.
    #[serde(flatten)]
    pub base: Attribute,
    /// Specifies if the attribute is mandatory. Can be "required",
    /// "conditionally_required", "recommended" or "opt_in". When omitted,
    /// the attribute is "recommended". When set to
    /// "conditionally_required", the string provided as `condition` MUST
    /// specify the conditions under which the attribute is required.
    pub requirement_level: RequirementLevel,

    /// Specifies if the attribute is (especially) relevant for sampling
    /// and thus should be set at span start. It defaults to false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling_relevant: Option<bool>,
}

/// A refinement of a span signal, for use in code-gen or specific library application.
///
/// A refinement represents a "view" of a Span that is highly optimised for a particular implementation.
/// e.g. for HTTP spans, there may be a refinement that provides only the necessary information for dealing with Java's HTTP
/// client library, and drops optional or extraneous information from the underlying http span.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct SpanRefinement {
    /// The identity of the refinement.
    pub id: SignalId,

    // TODO - This is a lazy way of doing this.  We use `type` to refer
    // to the underlying span definition, but override all fields here.
    // We probably should copy-paste all the "span" attributes here
    // including the `ty`
    /// The definition of the metric refinement.
    #[serde(flatten)]
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::Environment;
    use weaver_semconv::v2::span::{SpanName, SpanNameTemplate};

    #[test]
    fn test_forge_span_name_templates_jinja_rendering() {
        let span = Span {
            r#type: "http.client".into(),
            kind: SpanKindSpec::Client,
            name: SpanName {
                templates: vec![
                    SpanNameTemplate::parse("{http.request.method} {url.template}").unwrap(),
                    SpanNameTemplate::parse("{http.request.method} {server.address}:{server.port}")
                        .unwrap(),
                    SpanNameTemplate::parse("{http.request.method}").unwrap(),
                    SpanNameTemplate::parse("HTTP").unwrap(),
                ],
                note: None,
            },
            attributes: vec![],
            entity_associations: vec![],
            requirement_level: None,
            common: Default::default(),
            provenance: Default::default(),
        };

        let jinja_template = r#"
{%- for t in span.name.templates %}
{%- if t.attributes %}
{% if loop.first %}if{% else %}elif{% endif %} {% for attr in t.attributes %}"{{ attr }}" in attrs{% if not loop.last %} and {% endif %}{% endfor %}:
    return f"{% for p in t.parts %}{% if p.type == 'literal' %}{{ p.value }}{% else %}{attrs['{{ p.attribute }}']}{% endif %}{% endfor %}"
{%- else %}
else:
    return "{{ t.pattern }}"
{%- endif %}
{%- endfor %}
"#;

        let mut env = Environment::new();
        env.add_template("span_name.py.j2", jinja_template).unwrap();
        let tmpl = env.get_template("span_name.py.j2").unwrap();

        let rendered = tmpl.render(minijinja::context! { span => span }).unwrap();
        let expected = r#"if "http.request.method" in attrs and "url.template" in attrs:
    return f"{attrs['http.request.method']} {attrs['url.template']}"
elif "http.request.method" in attrs and "server.address" in attrs and "server.port" in attrs:
    return f"{attrs['http.request.method']} {attrs['server.address']}:{attrs['server.port']}"
elif "http.request.method" in attrs:
    return f"{attrs['http.request.method']}"
else:
    return "HTTP""#;

        assert_eq!(rendered.trim(), expected.trim());
    }
}
