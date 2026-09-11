// SPDX-License-Identifier: Apache-2.0

//! The new way we want to define spans going forward.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    deprecated::Deprecated,
    entity_association::EntityAssociation,
    signal_requirement_level::SignalRequirementLevel,
    stability::Stability,
    v2::{attribute::AttributeRef, signal_id::SignalId, CommonFields},
    YamlValue,
};

/// The span kind.
#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema, PartialOrd, Ord, Copy,
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum SpanKindSpec {
    /// An internal span.
    Internal,
    /// A client span.
    Client,
    /// A server span.
    Server,
    /// A producer span.
    Producer,
    /// A consumer span.
    Consumer,
}

impl Display for SpanKindSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SpanKindSpec::Internal => write!(f, "internal"),
            SpanKindSpec::Server => write!(f, "server"),
            SpanKindSpec::Client => write!(f, "client"),
            SpanKindSpec::Producer => write!(f, "producer"),
            SpanKindSpec::Consumer => write!(f, "consumer"),
        }
    }
}

/// A reference to an attribute group for spans.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpanGroupRef {
    /// Reference an existing attribute group by id.
    pub ref_group: String,
}

/// A parsed component of a span name template: either a literal string or an attribute reference.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Eq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemplatePart {
    /// A literal string component.
    Literal {
        /// The literal string content.
        value: String,
    },
    /// An attribute reference component.
    Attribute {
        /// The attribute key.
        attribute: String,
    },
}

/// A parsed span name template pattern.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SpanNameTemplate {
    /// The original pattern string, e.g. "{http.request.method} {url.template}".
    pub pattern: String,
    /// The list of attribute keys required by this template.
    pub attributes: Vec<String>,
    /// The parsed components of the template.
    pub parts: Vec<TemplatePart>,
}

impl SpanNameTemplate {
    /// Parses a template pattern string into a `SpanNameTemplate`.
    ///
    /// Placeholders are delimited by `{` and `}` and contain the attribute key.
    /// Text outside braces is treated as literal delimiters.
    pub fn parse(pattern: &str) -> Result<Self, String> {
        if pattern.trim().is_empty() {
            return Err("Span name template pattern cannot be empty".to_owned());
        }

        let mut parts = Vec::new();
        let mut attributes = Vec::new();
        let mut chars = pattern.chars().peekable();
        let mut current_literal = String::new();

        while let Some(c) = chars.next() {
            if c == '{' {
                let mut attr = String::new();
                let mut closed = false;
                for inner in chars.by_ref() {
                    if inner == '}' {
                        closed = true;
                        break;
                    }
                    if inner == '{' {
                        return Err(format!("Nested '{{' in template: `{pattern}`"));
                    }
                    attr.push(inner);
                }
                if !closed {
                    return Err(format!("Unclosed '{{' in template: `{pattern}`"));
                }
                if attr.is_empty()
                    || attr.contains(char::is_whitespace)
                    || !attr
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
                {
                    return Err(format!(
                        "Invalid attribute placeholder `{attr}` in template: `{pattern}`"
                    ));
                }
                if !current_literal.is_empty() {
                    parts.push(TemplatePart::Literal {
                        value: std::mem::take(&mut current_literal),
                    });
                }
                if !attributes.iter().any(|a| a == &attr) {
                    attributes.push(attr.to_owned());
                }
                parts.push(TemplatePart::Attribute {
                    attribute: attr.to_owned(),
                });
            } else if c == '}' {
                return Err(format!("Unmatched '}}' in template: `{pattern}`"));
            } else {
                current_literal.push(c);
            }
        }

        if !current_literal.is_empty() {
            parts.push(TemplatePart::Literal {
                value: current_literal,
            });
        }

        Ok(Self {
            pattern: pattern.to_owned(),
            attributes,
            parts,
        })
    }

    /// Renders the template given an attribute value lookup function.
    /// Returns `None` if any required attribute is missing or empty.
    pub fn render<'a, F>(&self, mut get_attr: F) -> Option<String>
    where
        F: FnMut(&str) -> Option<&'a str>,
    {
        let mut result = String::new();
        for part in &self.parts {
            match part {
                TemplatePart::Literal { value } => result.push_str(value),
                TemplatePart::Attribute { attribute } => {
                    let val = get_attr(attribute)?;
                    if val.is_empty() {
                        return None;
                    }
                    result.push_str(val);
                }
            }
        }
        Some(result)
    }
}

impl Display for SpanNameTemplate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pattern)
    }
}

impl TryFrom<String> for SpanNameTemplate {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl TryFrom<&str> for SpanNameTemplate {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl<'de> Deserialize<'de> for SpanNameTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SpanNameTemplateVisitor;

        impl<'de> serde::de::Visitor<'de> for SpanNameTemplateVisitor {
            type Value = SpanNameTemplate;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a span name template string or object")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                SpanNameTemplate::parse(value).map_err(serde::de::Error::custom)
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut pattern: Option<String> = None;
                let mut attributes: Option<Vec<String>> = None;
                let mut parts: Option<Vec<TemplatePart>> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "pattern" => pattern = Some(map.next_value()?),
                        "attributes" => attributes = Some(map.next_value()?),
                        "parts" => parts = Some(map.next_value()?),
                        unknown => {
                            return Err(serde::de::Error::unknown_field(
                                unknown,
                                &["pattern", "attributes", "parts"],
                            ));
                        }
                    }
                }

                let pattern: String =
                    pattern.ok_or_else(|| serde::de::Error::missing_field("pattern"))?;
                let parsed = SpanNameTemplate::parse(&pattern).map_err(serde::de::Error::custom)?;
                if let Some(attrs) = attributes {
                    if attrs != parsed.attributes {
                        return Err(serde::de::Error::custom(
                            "provided 'attributes' does not match attributes extracted from 'pattern'",
                        ));
                    }
                }
                if let Some(p) = parts {
                    if p != parsed.parts {
                        return Err(serde::de::Error::custom(
                            "provided 'parts' does not match parts extracted from 'pattern'",
                        ));
                    }
                }
                Ok(parsed)
            }
        }

        deserializer.deserialize_any(SpanNameTemplateVisitor)
    }
}

impl JsonSchema for SpanNameTemplate {
    fn schema_name() -> Cow<'static, str> {
        "SpanNameTemplate".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::SpanNameTemplate").into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let part_schema = generator.subschema_for::<TemplatePart>();
        schemars::json_schema!({
            "oneOf": [
                { "type": "string" },
                {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string" },
                        "attributes": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "parts": {
                            "type": "array",
                            "items": part_schema
                        }
                    },
                    "required": ["pattern"],
                    "additionalProperties": false
                }
            ]
        })
    }
}

/// Specification of the span name.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq, Eq, Default)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct SpanName {
    /// Ordered list of templates used to construct the span name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub templates: Vec<SpanNameTemplate>,
    /// Description of how a span name should be created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl SpanName {
    /// Evaluates the span name against an attribute lookup function.
    /// Iterates through `templates` in order, returning the first match.
    #[must_use]
    pub fn evaluate<'a, F>(&self, mut get_attr: F) -> Option<String>
    where
        F: FnMut(&str) -> Option<&'a str>,
    {
        for template in &self.templates {
            if let Some(rendered) = template.render(&mut get_attr) {
                return Some(rendered);
            }
        }
        None
    }

    /// Evaluates the span name against an attribute lookup function, falling back
    /// to `default_name` (e.g. the span's type or operation name) if no template matches.
    #[must_use]
    pub fn evaluate_or_default<'a, F>(&self, default_name: &str, get_attr: F) -> String
    where
        F: FnMut(&str) -> Option<&'a str>,
    {
        self.evaluate(get_attr)
            .unwrap_or_else(|| default_name.to_owned())
    }
}

/// A refinement of an Attribute for a span.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct SpanAttributeRef {
    /// Baseline attribute reference.
    #[serde(flatten)]
    pub base: AttributeRef,
    /// Specifies if the attribute is (especially) relevant for sampling
    /// and thus should be set at span start. It defaults to false.
    /// Note: this field is experimental.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling_relevant: Option<bool>,
}

/// A reference to either a span attribute or an attribute group.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum SpanAttributeOrGroupRef {
    /// Reference to a span attribute.
    Attribute(SpanAttributeRef),
    /// Reference to an attribute group.
    Group(SpanGroupRef),
}

/// Helper function to split a vector of SpanAttributeOrGroupRef into separate vectors
/// of SpanAttributeRef and group reference strings
#[must_use]
pub fn split_span_attributes_and_groups(
    attributes: Vec<SpanAttributeOrGroupRef>,
) -> (Vec<SpanAttributeRef>, Vec<String>) {
    let mut attribute_refs = Vec::new();
    let mut groups = Vec::new();

    for item in attributes {
        match item {
            SpanAttributeOrGroupRef::Attribute(attr_ref) => {
                attribute_refs.push(attr_ref);
            }
            SpanAttributeOrGroupRef::Group(group_ref) => {
                groups.push(group_ref.ref_group);
            }
        }
    }

    (attribute_refs, groups)
}

/// Defines a new Span signal.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Span {
    /// The type of the Span. This denotes the identity
    /// of the "shape" of this span, and must be unique.
    pub r#type: SignalId,
    /// Specifies the kind of the span.
    /// Note: only valid if type is span
    pub kind: SpanKindSpec,
    /// The name pattern for the span.
    pub name: SpanName,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<SpanAttributeOrGroupRef>,
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
}

/// A refinement of an existing span.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpanRefinement {
    /// The ID of the refinement.
    pub id: SignalId,
    /// The name of the span being refined.
    pub r#ref: SignalId,
    /// Overrides the span name specification from the referenced base span.
    /// If set, the entire `name` structure from the refinement replaces the
    /// base span's `name`; otherwise, the base span's `name` is inherited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<SpanName>,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<SpanAttributeOrGroupRef>,
    /// Which resources this span should be associated with.
    ///
    /// The list is an implicit `one_of` (telemetry must satisfy at least one entry); each entry is an
    /// entity reference or a nested `one_of`/`all_of` expression.
    /// Note: This field is currently not propagated during resolution.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entity_associations: Vec<EntityAssociation>,

    /// Refines the brief description of the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// Refines the more elaborate description of the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Refines the stability of the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the signal is deprecated.
    /// Note: This field is currently not propagated during resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<Deprecated>,
    /// Additional annotations for the signal.
    /// Note: This field is currently not propagated during resolution.
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, YamlValue>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::attribute::AttributeRef;

    #[test]
    fn test_span_attribute_ref_rejects_stability_and_deprecated() {
        for (field, name) in [
            ("stability: stable", "stability"),
            ("deprecated:\n  reason: obsoleted", "deprecated"),
        ] {
            let yaml = format!("ref: my.attribute\n{field}\n");
            let err = serde_yaml::from_str::<SpanAttributeRef>(&yaml)
                .expect_err("stability/deprecated must not be allowed on span attribute refs");
            assert!(err.to_string().contains(&format!("unknown field `{name}`")));
        }
    }

    #[test]
    fn test_span_kind_spec_display() {
        assert_eq!(SpanKindSpec::Internal.to_string(), "internal");
        assert_eq!(SpanKindSpec::Server.to_string(), "server");
        assert_eq!(SpanKindSpec::Client.to_string(), "client");
        assert_eq!(SpanKindSpec::Producer.to_string(), "producer");
        assert_eq!(SpanKindSpec::Consumer.to_string(), "consumer");
    }

    #[test]
    fn test_split_span_attributes_and_groups() {
        let items = vec![
            SpanAttributeOrGroupRef::Attribute(SpanAttributeRef {
                base: AttributeRef {
                    r#ref: "http.status".to_owned(),
                    brief: None,
                    examples: None,
                    requirement_level: None,
                    note: None,
                    annotations: Default::default(),
                },
                sampling_relevant: Some(true),
            }),
            SpanAttributeOrGroupRef::Group(SpanGroupRef {
                ref_group: "http.shared".to_owned(),
            }),
        ];

        let (attrs, groups) = split_span_attributes_and_groups(items);
        assert_eq!(attrs.len(), 1);
        assert_eq!(groups.len(), 1);
        assert_eq!(attrs[0].base.r#ref, "http.status");
        assert_eq!(attrs[0].sampling_relevant, Some(true));
        assert_eq!(groups[0], "http.shared");
    }

    #[test]
    fn test_span_name_template_parsing() {
        let t = SpanNameTemplate::parse("{http.request.method} {url.template}").unwrap();
        assert_eq!(t.pattern, "{http.request.method} {url.template}");
        assert_eq!(t.attributes, vec!["http.request.method", "url.template"]);
        assert_eq!(
            t.parts,
            vec![
                TemplatePart::Attribute {
                    attribute: "http.request.method".to_owned()
                },
                TemplatePart::Literal {
                    value: " ".to_owned()
                },
                TemplatePart::Attribute {
                    attribute: "url.template".to_owned()
                },
            ]
        );

        let literal_only = SpanNameTemplate::parse("HTTP").unwrap();
        assert!(literal_only.attributes.is_empty());
        assert_eq!(
            literal_only.parts,
            vec![TemplatePart::Literal {
                value: "HTTP".to_owned()
            }]
        );

        assert!(SpanNameTemplate::parse("").is_err());
        assert!(SpanNameTemplate::parse("   ").is_err());
        assert!(SpanNameTemplate::parse("{unclosed").is_err());
        assert!(SpanNameTemplate::parse("{}").is_err());
        assert!(SpanNameTemplate::parse("{   }").is_err());
        assert!(SpanNameTemplate::parse("{a b}").is_err());
        assert!(SpanNameTemplate::parse("{ a }").is_err());
        assert!(SpanNameTemplate::parse("{invalid!attr}").is_err());
        assert!(SpanNameTemplate::parse("stray}brace").is_err());
        assert!(SpanNameTemplate::parse("{nested{brace}}").is_err());

        // Consecutive placeholders
        let consecutive = SpanNameTemplate::parse("{a}{b}").unwrap();
        assert_eq!(consecutive.attributes, vec!["a", "b"]);
        assert_eq!(consecutive.parts.len(), 2);
    }

    #[test]
    fn test_span_name_template_rendering() {
        let t = SpanNameTemplate::parse("{method} {url.template}").unwrap();

        // All present and valid
        let rendered = t.render(|key| match key {
            "method" => Some("GET"),
            "url.template" => Some("/users/{id}"),
            _ => None,
        });
        assert_eq!(rendered.as_deref(), Some("GET /users/{id}"));

        // Missing attribute
        let rendered = t.render(|key| match key {
            "method" => Some("GET"),
            _ => None,
        });
        assert_eq!(rendered, None);

        // Empty value is rejected
        let rendered = t.render(|key| match key {
            "method" => Some(""),
            "url.template" => Some("/users/{id}"),
            _ => None,
        });
        assert_eq!(rendered, None);
    }

    #[test]
    fn test_span_name_evaluation() {
        let span_name = SpanName {
            templates: vec![
                SpanNameTemplate::parse("{http.request.method} {url.template}").unwrap(),
                SpanNameTemplate::parse("{http.request.method} {server.address}:{server.port}")
                    .unwrap(),
                SpanNameTemplate::parse("{http.request.method} {server.address}").unwrap(),
                SpanNameTemplate::parse("{http.request.method}").unwrap(),
                SpanNameTemplate::parse("HTTP").unwrap(),
            ],
            note: None,
        };

        // First template matches
        let name = span_name.evaluate(|key| match key {
            "http.request.method" => Some("GET"),
            "url.template" => Some("/users/{id}"),
            _ => None,
        });
        assert_eq!(name.as_deref(), Some("GET /users/{id}"));

        // First template fails, second matches
        let name = span_name.evaluate(|key| match key {
            "http.request.method" => Some("POST"),
            "server.address" => Some("example.com"),
            "server.port" => Some("8080"),
            _ => None,
        });
        assert_eq!(name.as_deref(), Some("POST example.com:8080"));

        // Only method matches
        let name = span_name.evaluate(|key| match key {
            "http.request.method" => Some("DELETE"),
            _ => None,
        });
        assert_eq!(name.as_deref(), Some("DELETE"));

        // _OTHER for enum http.request.method causes method templates to fail, matches literal "HTTP" template
        let name = span_name.evaluate(|key| match key {
            "http.request.method" => Some("_OTHER").filter(|v| *v != "_OTHER"),
            "url.template" => Some("/users/{id}"),
            _ => None,
        });
        assert_eq!(name.as_deref(), Some("HTTP"));

        // Non-enum attribute with value "_OTHER" is preserved
        let custom_span = SpanName {
            templates: vec![SpanNameTemplate::parse("{url.template}").unwrap()],
            note: None,
        };
        let name = custom_span.evaluate(|key| match key {
            "url.template" => Some("_OTHER"),
            _ => None,
        });
        assert_eq!(name.as_deref(), Some("_OTHER"));

        // Completely empty attributes matches literal "HTTP" template
        let name = span_name.evaluate(|_| None);
        assert_eq!(name.as_deref(), Some("HTTP"));
    }

    #[test]
    fn test_span_name_deserialization() {
        let yaml = r#"
templates:
  - "{http.request.method} {url.template}"
  - "{http.request.method}"
  - "HTTP"
note: "test note"
"#;
        let parsed: SpanName = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(parsed.templates.len(), 3);
        assert_eq!(
            parsed.templates[0].pattern,
            "{http.request.method} {url.template}"
        );
        assert_eq!(parsed.templates[2].pattern, "HTTP");
        assert_eq!(parsed.note.as_deref(), Some("test note"));

        // Backwards compatibility: note only
        let yaml_note_only = "note: free form note\n";
        let parsed_note: SpanName = serde_yaml::from_str(yaml_note_only).unwrap();
        assert!(parsed_note.templates.is_empty());
        assert_eq!(parsed_note.note.as_deref(), Some("free form note"));

        // Roundtrip serialization: templates serialize as objects with pattern, attributes, parts
        let serialized = serde_yaml::to_string(&parsed).unwrap();
        let roundtrip: SpanName = serde_yaml::from_str(&serialized).unwrap();
        assert_eq!(roundtrip, parsed);

        // JSON roundtrip
        let json_serialized = serde_json::to_string(&parsed).unwrap();
        let json_roundtrip: SpanName = serde_json::from_str(&json_serialized).unwrap();
        assert_eq!(json_roundtrip, parsed);

        // Unknown fields rejected
        let invalid_yaml = r#"
pattern: "HTTP"
unknown_key: "val"
"#;
        assert!(serde_yaml::from_str::<SpanNameTemplate>(invalid_yaml).is_err());

        // Inconsistent attributes rejected
        let inconsistent_yaml = r#"
pattern: "{a}"
attributes: ["b"]
"#;
        assert!(serde_yaml::from_str::<SpanNameTemplate>(inconsistent_yaml).is_err());
    }
}
