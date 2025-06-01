// SPDX-License-Identifier: Apache-2.0

//! Translations from Weaver to Otel for metrics.

use crate::spans::get_attribute_name_value;
use opentelemetry::global;
use weaver_forge::registry::ResolvedRegistry;
use weaver_semconv::group::GroupType;
use weaver_semconv::group::InstrumentSpec;

/// Uses the global meter_provider to emit metrics for all the defined
/// metrics in the registry
pub(crate) fn emit_metrics_for_registry(registry: &ResolvedRegistry) {
    let meter = global::meter("weaver");

    // Emit each metric to the OTLP receiver.
    for group in registry.groups.iter() {
        if group.r#type == GroupType::Metric {
            if let Some(instrument) = &group.instrument {
                let metric_name = group.metric_name.clone().unwrap_or("".to_owned());
                let unit = group.unit.clone().unwrap_or("".to_owned());
                let description = group.brief.clone();

                let attributes = group
                    .attributes
                    .iter()
                    .map(get_attribute_name_value)
                    .collect::<Vec<_>>();

                match instrument {
                    InstrumentSpec::UpDownCounter => {
                        let up_down_counter = meter
                            .i64_up_down_counter(metric_name)
                            .with_unit(unit)
                            .with_description(description)
                            .build();
                        up_down_counter.add(1, &attributes);
                    }
                    InstrumentSpec::Counter => {
                        let counter = meter
                            .u64_counter(metric_name)
                            .with_unit(unit)
                            .with_description(description)
                            .build();
                        counter.add(1, &attributes);
                    }
                    InstrumentSpec::Gauge => {
                        let gauge = meter
                            .i64_gauge(metric_name)
                            .with_unit(unit)
                            .with_description(description)
                            .build();
                        gauge.record(1, &attributes);
                    }
                    InstrumentSpec::Histogram => {
                        let histogram = meter
                            .u64_histogram(metric_name)
                            .with_unit(unit)
                            .with_description(description)
                            .build();
                        histogram.record(1, &attributes);
                    }
                }
            }
        }
    }
}
