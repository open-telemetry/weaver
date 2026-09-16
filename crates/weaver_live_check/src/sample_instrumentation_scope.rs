// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for instrumentation scope metadata.

use std::rc::Rc;

use cel::{Context, SerializationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    cel::{attribute_map, Matchable},
    live_checker::LiveChecker,
    matcher::SampleMatch,
    sample_attribute::SampleAttribute,
    Advisable, Error, LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, Sample, SampleRef,
    SampleType,
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
        let sample_match = Rc::new(live_checker.match_for(self, None));
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

impl Matchable for SampleInstrumentationScope {
    fn sample_type(&self) -> SampleType {
        SampleType::InstrumentationScope
    }

    fn bind(&self, context: &mut Context<'_>) -> Result<(), SerializationError> {
        context.add_variable("name", &self.name)?;
        context.add_variable("version", &self.version)?;
        context.add_variable("schema_url", &self.schema_url)?;
        context.add_variable("attributes", attribute_map(self.attributes.iter()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cel::evaluate;

    #[test]
    fn a_scope_binds_its_fields() {
        let scope: SampleInstrumentationScope = serde_json::from_str(include_str!(
            "../fixtures/cel/instrumentation-scope/scope-myapp-checkout.json"
        ))
        .expect("the fixture parses");
        let when = r#"name == "myapp.checkout.instrumentation" && version == "0.3.1"
            && schema_url == "https://example.com/myschema/1.0.0"
            && attributes["myapp.instrumentation.mode"] == "auto""#;
        assert!(evaluate(when, &scope).expect("it evaluates"));
    }
}
