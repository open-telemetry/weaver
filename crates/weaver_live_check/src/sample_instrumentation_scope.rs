// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for instrumentation scope metadata.

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    live_checker::LiveChecker, matcher::SampleMatch, sample_attribute::SampleAttribute, Advisable,
    Error, LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, Sample, SampleRef,
};

/// Identifies the instrumentation scope that produced a telemetry signal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleInstrumentationScope {
    /// Instrumentation scope name.
    #[serde(default)]
    pub name: String,
    /// Instrumentation scope version.
    #[serde(default)]
    pub version: String,
    /// Schema URL declared by the OTLP scope container.
    #[serde(default)]
    pub schema_url: String,
    /// Instrumentation scope attributes.
    #[serde(default)]
    pub attributes: Vec<SampleAttribute>,
    /// Number of scope attributes dropped before export.
    #[serde(default)]
    pub dropped_attributes_count: u32,
    /// Live check result.
    pub live_check_result: Option<LiveCheckResult>,
}

impl Advisable for SampleInstrumentationScope {
    fn as_sample_ref(&self) -> SampleRef<'_> {
        SampleRef::InstrumentationScope(self)
    }

    fn entity_type(&self) -> &str {
        "instrumentation_scope"
    }
}

impl LiveCheckRunner for SampleInstrumentationScope {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent: Option<Rc<SampleMatch>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        self.live_check_result =
            Some(self.run_advisors(live_checker, stats, parent.clone(), parent_signal)?);
        self.attributes
            .run_live_check(live_checker, stats, parent, parent_signal)
    }
}
