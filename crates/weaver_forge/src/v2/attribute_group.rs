//! Version two of attribute groups.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use weaver_semconv::v2::{signal_id::SignalId, CommonFields};

use crate::v2::attribute::Attribute;

/// Public attribute group.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttributeGroup {
    /// The name of the attribute group, must be unique.
    pub id: SignalId,
    /// List of attributes.
    pub attributes: Vec<Attribute>,
    /// Common fields (like brief, note, annotations).
    #[serde(flatten)]
    pub common: CommonFields,
}
