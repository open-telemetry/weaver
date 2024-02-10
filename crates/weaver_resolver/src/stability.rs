// SPDX-License-Identifier: Apache-2.0

//! Functions to resolve a semantic convention stability field.

use weaver_semconv::stability::StabilitySpec;

pub fn resolve_stability(
    stability: &Option<StabilitySpec>,
) -> Option<weaver_resolved_schema::catalog::Stability> {
    stability.as_ref().map(|stability| match stability {
        StabilitySpec::Deprecated => weaver_resolved_schema::catalog::Stability::Deprecated,
        StabilitySpec::Experimental => weaver_resolved_schema::catalog::Stability::Experimental,
        StabilitySpec::Stable => weaver_resolved_schema::catalog::Stability::Stable,
    })
}
