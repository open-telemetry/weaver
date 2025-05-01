// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample resources

use serde::{Deserialize, Serialize};

use crate::{
    live_checker::LiveChecker, sample_attribute::SampleAttribute, Error, LiveCheckResult,
    LiveCheckRunner, LiveCheckStatistics, SampleRef,
};

/// Represents a resource
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleResource {
    /// The attributes of the resource
    #[serde(default)]
    pub attributes: Vec<SampleAttribute>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl LiveCheckRunner for SampleResource {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
    ) -> Result<(), Error> {
        let mut result = LiveCheckResult::new();
        for advisor in live_checker.advisors.iter_mut() {
            let advice_list = advisor.advise(SampleRef::Resource(self), None, None)?;
            result.add_advice_list(advice_list);
        }
        for attribute in &mut self.attributes {
            attribute.run_live_check(live_checker, stats)?;
        }
        self.live_check_result = Some(result);
        stats.inc_entity_count("resource");
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        Ok(())
    }
}
