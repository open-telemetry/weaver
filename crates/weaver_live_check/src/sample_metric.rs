// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample spans

use serde::{Deserialize, Serialize};
use serde_json::Value;
use weaver_checker::violation::{Advice, AdviceLevel};
use weaver_semconv::group::InstrumentSpec;

use crate::{
    live_checker::LiveChecker, sample_attribute::SampleAttribute, Error, LiveCheckResult,
    LiveCheckRunner, LiveCheckStatistics, SampleRef, MISSING_METRIC_ADVICE_TYPE,
};

/// Represents a sample telemetry span parsed from any source
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleMetric {
    /// Metric name.
    pub name: String,
    /// Set of attributes
    //    #[serde(default)]
    //    pub attributes: Vec<SampleAttribute>,
    /// Type of the metric (e.g. gauge, histogram, ...).
    pub instrument: InstrumentSpec,
    /// Unit of the metric.
    pub unit: String,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
}

impl LiveCheckRunner for SampleMetric {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
    ) -> Result<(), Error> {
        let mut result = LiveCheckResult::new();
        // find the metric in the registry
        let semconv_metric = live_checker.find_metric(&self.name);
        if semconv_metric.is_none() {
            result.add_advice(Advice {
                advice_type: MISSING_METRIC_ADVICE_TYPE.to_owned(),
                value: Value::String(self.name.clone()),
                message: "Does not exist in the registry".to_owned(),
                advice_level: AdviceLevel::Violation,
            });
        };
        for advisor in live_checker.advisors.iter_mut() {
            let advice_list =
                advisor.advise(SampleRef::Metric(self), None, semconv_metric.as_ref())?;
            result.add_advice_list(advice_list);
        }
        // TODO for each data point...

        self.live_check_result = Some(result);
        stats.inc_entity_count("metric");
        stats.maybe_add_live_check_result(self.live_check_result.as_ref());
        Ok(())
    }
}
