// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample resources

use serde::{Deserialize, Serialize};

use crate::{
    advice::Advisor, live_checker::LiveChecker, sample_attribute::SampleAttribute, LiveCheckResult,
    LiveCheckRunner, LiveCheckStatistics,
};

/// Represents a resource
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleResource {
    /// The attributes of the resource
    pub attributes: Vec<SampleAttribute>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl LiveCheckRunner for SampleResource {
    fn run_live_check(&mut self, live_checker: &mut LiveChecker, stats: &mut LiveCheckStatistics) {
        let mut result = LiveCheckResult::new();
        for entity_advisor in live_checker.advisors.iter_mut() {
            if let Advisor::Resource(advisor) = entity_advisor {
                if let Ok(advice_list) = advisor.advise(self, None) {
                    result.add_advice_list(advice_list);
                }
            }
        }
        for attribute in &mut self.attributes {
            attribute.run_live_check(live_checker, stats);
        }
        self.live_check_result = Some(result);
    }
}
