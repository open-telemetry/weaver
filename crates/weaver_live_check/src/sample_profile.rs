// SPDX-License-Identifier: Apache-2.0

//! Intermediary format for telemetry sample profiles

use std::rc::Rc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    live_checker::LiveChecker, matcher::SampleMatch, sample_attribute::SampleAttribute, Advisable,
    Error, LiveCheckResult, LiveCheckRunner, LiveCheckStatistics, Sample,
    SampleInstrumentationScope, SampleRef, SampleResource, SampleType,
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
        _parent: Option<Rc<SampleMatch>>,
        parent_signal: &Sample,
    ) -> Result<(), Error> {
        let sample_match = Rc::new(live_checker.match_for(SampleType::Profile, self, None));
        live_checker.record_match(&sample_match);
        let mut result = self.run_advisors(
            live_checker,
            stats,
            Some(Rc::clone(&sample_match)),
            parent_signal,
        )?;
        sample_match.add_findings(
            &SampleRef::Profile(self),
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use weaver_forge::v2::registry::{ForgeResolvedRegistry, Refinements, Registry};

    use super::*;
    use crate::{
        live_checker::LiveChecker, matcher::fixture::matcher_configs, DisabledStatistics,
        LiveCheckStatistics, Sample, SampleRef, VersionedRegistry,
    };

    fn make_profile() -> SampleProfile {
        SampleProfile {
            original_payload_format: "pprof".to_owned(),
            attributes: vec![],
            instrumentation_scope: None,
            live_check_result: None,
            resource: None,
        }
    }

    fn empty_registry() -> Arc<VersionedRegistry> {
        Arc::new(VersionedRegistry::V2(Box::new(ForgeResolvedRegistry {
            schema_url: "https://example.com/1.0.0".try_into().unwrap(),
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: vec![],
            },
            refinements: Refinements {
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: Default::default(),
            dependency_graph: Default::default(),
        })))
    }

    #[test]
    fn test_entity_type() {
        assert_eq!(make_profile().entity_type(), "profile");
    }

    #[test]
    fn test_as_sample_ref() {
        let profile = make_profile();
        assert!(matches!(profile.as_sample_ref(), SampleRef::Profile(_)));
    }

    /// The OpenTelemetry SDKs cannot emit profiles yet, so this is the only
    /// coverage of a profile matcher.
    #[test]
    fn a_profile_matcher_matches_a_profile() {
        let mut profile = make_profile();
        let mut live_checker = LiveChecker::new(empty_registry(), vec![]);
        live_checker
            .set_matchers(&matcher_configs(
                r#"
[[live-check.matchers]]
id = "acme.profile"
sample_type = "profile"
when = 'true'
"#,
            ))
            .expect("the matchers compile");
        let mut stats = LiveCheckStatistics::Disabled(DisabledStatistics);
        let parent = Sample::Profile(make_profile());
        profile
            .run_live_check(&mut live_checker, &mut stats, None, &parent)
            .expect("the profile is checked");
        let matcher = live_checker
            .matchers()
            .iter()
            .next()
            .expect("there is one matcher");
        assert_eq!(matcher.matched(), 1);
    }

    #[test]
    fn test_run_live_check_no_advisors() {
        let mut profile = make_profile();
        let mut live_checker = LiveChecker::new(empty_registry(), vec![]);
        let mut stats = LiveCheckStatistics::Disabled(DisabledStatistics);
        let parent = Sample::Profile(make_profile());
        let result = profile.run_live_check(&mut live_checker, &mut stats, None, &parent);
        assert!(result.is_ok());
        assert!(profile.live_check_result.is_some());
    }
}
