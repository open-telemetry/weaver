// SPDX-License-Identifier: Apache-2.0

//! Signal identifier type for version 2 semantic conventions.

use std::fmt::{self, Display, Formatter};
use std::ops::Deref;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

/// An identifier for a signal.  Should be `.` separated namespaces and names.
#[derive(Serialize, JsonSchema, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SignalId(String);

impl SignalId {
    /// Returns the v1 version of signal ids (raw strings).
    #[must_use]
    pub fn into_v1(self) -> String {
        self.0
    }
}

impl From<String> for SignalId {
    fn from(value: String) -> Self {
        SignalId(value)
    }
}

impl From<&str> for SignalId {
    fn from(value: &str) -> Self {
        SignalId(value.to_owned())
    }
}

impl Deref for SignalId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for SignalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Custom deserializer reserved for future SignalId format validation.
impl<'de> Deserialize<'de> for SignalId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        // TODO - Enforce some constraints on allowed strings here...
        Ok(SignalId(s))
    }
}
