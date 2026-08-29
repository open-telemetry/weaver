// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample resources

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    live_checker::LiveChecker, matcher::SampleMatch, sample_attribute::SampleAttribute, Advisable,
    Error, LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, Sample, SampleRef, SampleType,
};

/// Represents a resource
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleResource {
    /// The attributes of the resource
    #[serde(default)]
    pub attributes: Vec<SampleAttribute>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl Advisable for SampleResource {
    fn as_sample_ref(&self) -> SampleRef<'_> {
        SampleRef::Resource(self)
    }

    fn entity_type(&self) -> &str {
        "resource"
    }
}

impl LiveCheckRunner for SampleResource {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        _parent: Option<Rc<SampleMatch>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        let sample_match = Rc::new(live_checker.match_for(SampleType::Resource, self, None));
        live_checker.record_match(&sample_match);
        let mut result = self.run_advisors(
            live_checker,
            stats,
            Some(Rc::clone(&sample_match)),
            parent_signal,
        )?;
        sample_match.add_findings(
            &SampleRef::Resource(self),
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
