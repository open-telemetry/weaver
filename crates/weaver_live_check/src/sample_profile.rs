// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample profiles

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    live_checker::LiveChecker, sample_attribute::SampleAttribute, Advisable, Error,
    LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, Sample, SampleInstrumentationScope,
    SampleRef, SampleResource, VersionedSignal,
};

/// Represents a profile collected via OTLP (v1development)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleProfile {
    /// Format of the original payload (e.g. "pprof", "jfr")
    pub original_payload_format: String,
    /// Attributes resolved from the ProfilesDictionary attribute table
    #[serde(default)]
    pub attributes: Vec<SampleAttribute>,
    /// Shared instrumentation scope that produced this profile (not serialized)
    #[serde(skip)]
    pub instrumentation_scope: Option<Rc<SampleInstrumentationScope>>,
    /// Live check result
    pub live_check_result: Option<LiveCheckResult>,
    /// Reference to the parent resource (not serialized)
    #[serde(skip)]
    pub resource: Option<Rc<SampleResource>>,
}

impl Advisable for SampleProfile {
    fn as_sample_ref(&self) -> SampleRef<'_> {
        SampleRef::Profile(self)
    }

    fn entity_type(&self) -> &str {
        "profile"
    }
}

impl LiveCheckRunner for SampleProfile {
    fn run_live_check(
        &mut self,
        live_checker: &mut LiveChecker,
        stats: &mut LiveCheckStatistics,
        parent_group: Option<Rc<VersionedSignal>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        self.live_check_result =
            Some(self.run_advisors(live_checker, stats, parent_group.clone(), parent_signal)?);
        self.attributes
            .run_live_check(live_checker, stats, parent_group, parent_signal)?;
        Ok(())
    }
}
