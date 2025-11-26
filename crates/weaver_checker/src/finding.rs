// SPDX-License-Identifier: Apache-2.0

//! Definition of a policy violation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::{Display, Formatter};

const SEMCONV_ATTRIBUTE: &str = "semconv_attribute";

/// Enum representing the different types of findings from enforcement policies.
#[derive(Debug, Clone, Serialize, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub struct PolicyFinding {
    /// The id of violation e.g. "is_deprecated". This should be a short,
    /// machine-readable string that categorizes the finding.
    pub id: String,

    /// The context associated with the finding e.g. { "attribute_name": "foo.bar", "attribute_value": "bar" }
    /// The context should contain all dynamic parts of the message
    /// Context values may be used with custom templates and filters to customize reports.
    pub context: Value,

    /// The human-readable message of the finding e.g. "This attribute 'foo.bar' is deprecated, reason: 'use foo.baz'"
    /// The message, along with signal_name and signal_type, should contain enough information to understand the advice and
    /// identify the issue and how to fix it.
    /// Some of the values used in the message may be also present in the `context` field to support report customization.
    pub message: String,

    /// The level of the finding e.g. "violation", "informational"
    pub level: FindingLevel,

    /// The signal type the finding applies to: "span", "metric", "entity", "log" (aka "event"), or "profile"
    pub signal_type: Option<String>,

    /// The signal name the finding applies to e.g. "http.server.request.duration".
    pub signal_name: Option<String>,
}

impl PolicyFinding {
    pub(crate) fn new_semconv_attribute(
        id: String,
        category: String,
        group: String,
        attr: String,
    ) -> PolicyFinding {
        let ctx = serde_json::json!({
            "id": id,
            "category": category,
            "group": group,
            "attr": attr,
        });
        let message = format!("id={id}, category={category}, group={group}, attr={attr}");
        PolicyFinding {
            id: SEMCONV_ATTRIBUTE.to_owned(),
            context: ctx,
            message,
            level: FindingLevel::Violation,
            signal_type: None,
            signal_name: None,
        }
    }
}

impl<'de> Deserialize<'de> for PolicyFinding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(ViolationBuilder::new())
    }
}

#[derive(Debug, Default)]
struct ViolationBuilder {}
impl ViolationBuilder {
    fn new() -> Self {
        Default::default()
    }
}
impl<'de> serde::de::Visitor<'de> for ViolationBuilder {
    type Value = PolicyFinding;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "A policy violation")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: serde::de::MapAccess<'de>,
    {
        // We have a custom deserializer that allows *any of* the following:
        // - Oldschool semconv_attribute
        // - Oldschool Advice
        // - New "unified finding" structure.
        let mut opt_id: Option<String> = None;
        let mut opt_category: Option<String> = None;
        let mut opt_group: Option<String> = None;
        let mut opt_attr: Option<String> = None;
        let mut opt_advice_type: Option<String> = None;
        let mut opt_message: Option<String> = None;
        let mut opt_level: Option<FindingLevel> = None;
        let mut opt_signal_type: Option<String> = None;
        let mut opt_signal_name: Option<String> = None;
        let mut opt_context: Option<Value> = None;
        let mut r#type: Option<String> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "type" => r#type = Some(map.next_value()?),
                "id" => opt_id = Some(map.next_value()?),
                "category" => opt_category = Some(map.next_value()?),
                "group" => opt_group = Some(map.next_value()?),
                "attr" => opt_attr = Some(map.next_value()?),
                "advice_type" => opt_advice_type = Some(map.next_value()?),
                "message" => opt_message = Some(map.next_value()?),
                "advice_level" => opt_level = Some(map.next_value()?),
                "level" => opt_level = Some(map.next_value()?),
                // TODO - ensure only one of advice_context or context are provided.
                "advice_context" => opt_context = Some(map.next_value()?),
                "context" => opt_context = Some(map.next_value()?),
                "signal_type" => opt_signal_type = Some(map.next_value()?),
                "signal_name" => opt_signal_name = Some(map.next_value()?),
                _ => (),
            }
        }
        match r#type.as_deref() {
            Some(SEMCONV_ATTRIBUTE) => {
                let id = opt_id.ok_or(serde::de::Error::missing_field("id"))?;
                let category = opt_category.ok_or(serde::de::Error::missing_field("category"))?;
                let group = opt_group.ok_or(serde::de::Error::missing_field("group"))?;
                let attr = opt_attr.ok_or(serde::de::Error::missing_field("attr"))?;
                // TODO - do we want a warning that this type is going away?
                Ok(PolicyFinding::new_semconv_attribute(id, category, group, attr))
            }
            Some("advice") => {
                // TODO - Should we warn that `type: advice` is no longer needed?
                let level = opt_level.ok_or(serde::de::Error::missing_field("advice_level"))?;
                let id = opt_advice_type.ok_or(serde::de::Error::missing_field("advice_type"))?;
                let message = opt_message.ok_or(serde::de::Error::missing_field("message"))?;
                let signal_type = opt_signal_type;
                let signal_name = opt_signal_name;
                let advice_context =
                    opt_context.ok_or(serde::de::Error::missing_field("advice_context"))?;
                Ok(PolicyFinding {
                    id,
                    context: advice_context,
                    message,
                    level,
                    signal_type,
                    signal_name,
                })
            }
            None => {
                // This is the one unified way to report errors going forward.
                let id = opt_id.ok_or(serde::de::Error::missing_field("id"))?;
                let level = opt_level.ok_or(serde::de::Error::missing_field("level"))?;
                let message = opt_message.ok_or(serde::de::Error::missing_field("message"))?;
                let signal_type = opt_signal_type;
                let signal_name = opt_signal_name;
                let context = opt_context.ok_or(serde::de::Error::missing_field("context"))?;
                Ok(PolicyFinding {
                    id,
                    context,
                    message,
                    level,
                    signal_type,
                    signal_name,
                })
            }
            _ => Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Map,
                &self,
            )),
        }
    }
}

impl Display for PolicyFinding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.id.as_str() {
            SEMCONV_ATTRIBUTE => write!(f, "{}", self.message),
            _ => write!(
                f,
                "id={}, context={}, message={}, level={:?}, signal_type={:?}, signal_name={:?}",
                self.id, self.context, self.message, self.level, self.signal_type, self.signal_name,
            ),
        }
    }
}

impl PolicyFinding {
    /// Returns the violation id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// The level of a finding.
#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, PartialOrd, Ord, Eq, Hash, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FindingLevel {
    /// Useful context without action needed.
    Information,
    /// Suggested change that would improve things.
    Improvement,
    /// Something that breaks compliance rules.
    Violation,
}
