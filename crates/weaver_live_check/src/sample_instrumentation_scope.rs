// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for instrumentation scope metadata.

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    live_checker::LiveChecker, matcher::SampleMatch, sample_attribute::SampleAttribute, Advisable,
    Error, LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, Sample, SampleRef, SampleType,
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
        _parent: Option<Rc<SampleMatch>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        let sample_match =
            Rc::new(live_checker.match_for(SampleType::InstrumentationScope, self, None));
        live_checker.record_match(&sample_match);
        let mut result = self.run_advisors(
            live_checker,
            stats,
            Some(Rc::clone(&sample_match)),
            parent_signal,
        )?;
        sample_match.add_findings(
            &SampleRef::InstrumentationScope(self),
            &self.attributes,
            &mut result,
            live_checker,
            parent_signal,
        );
        self.live_check_result = Some(result);
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        self.attributes
            .run_live_check(live_checker, stats, Some(sample_match), parent_signal)
    }
}
