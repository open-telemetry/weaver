// SPDX-License-Identifier: Apache-2.0

//! The new path forward for parsing identifiers for signals.

use std::{fmt, ops::Deref};

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Serialize, JsonSchema, Clone, Debug)]
/// An identifier for a signal.  Should be `.` separated namespaces and names.
pub struct SignalId(String);

impl SignalId {
    /// Returns the v1 version of signal ids (raw strings).
    #[must_use]
    pub fn into_v1(self) -> String {
        self.0
    }
}

// Allow `&SignalId` to be used for getting `&str`.
impl Deref for SignalId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Allow pretty printing.
impl fmt::Display for SignalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Custom deserializer we can use to, eventually, require a format for SignalId.
impl<'de> Deserialize<'de> for SignalId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        // TODO - Enforce some cosntraints on allowed strings here...
        Ok(SignalId(s))
    }
}
