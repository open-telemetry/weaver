// SPDX-License-Identifier: Apache-2.0

//! Resolve metric and metric_group

use crate::attribute::{merge_attributes, resolve_attributes};
use crate::Error;
use std::collections::{HashMap, HashSet};
use weaver_schema::attribute::to_schema_attributes;
use weaver_schema::metric_group::Metric;
use weaver_schema::schema_spec::SchemaSpec;
use weaver_schema::univariate_metric::UnivariateMetric;
use weaver_semconv::group::InstrumentSpec;
use weaver_semconv::SemConvSpecs;
use weaver_version::VersionChanges;

/// Resolves metrics and their attributes.
pub fn resolve_metrics(
    schema: &mut SchemaSpec,
    sem_conv_catalog: &SemConvSpecs,
    version_changes: &VersionChanges,
) -> Result<(), Error> {
    if let Some(metrics) = schema.resource_metrics.as_mut() {
        metrics.attributes = resolve_attributes(
            metrics.attributes.as_ref(),
            sem_conv_catalog,
            version_changes.metric_attribute_changes(),
        )?;

        // Resolve metrics (univariate)
        for metric in metrics.metrics.iter_mut() {
            if let UnivariateMetric::Ref {
                r#ref,
                attributes,
                tags,
            } = metric
            {
                *attributes = resolve_attributes(
                    attributes,
                    sem_conv_catalog,
                    version_changes.metric_attribute_changes(),
                )?;
                if let Some(referenced_metric) = sem_conv_catalog.metric(r#ref) {
                    let mut inherited_attrs = to_schema_attributes(&referenced_metric.attributes);
                    inherited_attrs = resolve_attributes(
                        &inherited_attrs,
                        sem_conv_catalog,
                        version_changes.metric_attribute_changes(),
                    )?;
                    let merged_attrs = merge_attributes(attributes, &inherited_attrs);
                    *metric = UnivariateMetric::Metric {
                        name: referenced_metric.name.clone(),
                        brief: referenced_metric.brief.clone(),
                        note: referenced_metric.note.clone(),
                        attributes: merged_attrs,
                        instrument: referenced_metric.instrument.clone(),
                        unit: referenced_metric.unit.clone(),
                        tags: tags.clone(),
                    };
                } else {
                    return Err(Error::FailToResolveMetric {
                        r#ref: r#ref.clone(),
                    });
                }
            }
        }

        // Resolve metric groups (multivariate metrics).
        // Attributes handling for the metrics present in the metric group:
        // - If the metrics share the same set of require attributes then all the attributes are
        // merged into the metric group attributes.
        // - Otherwise, an error is returned.
        for metrics in metrics.metric_groups.iter_mut() {
            let mut metric_group_attrs = HashMap::new();

            // Resolve metric group attributes
            resolve_attributes(
                metrics.attributes.as_ref(),
                sem_conv_catalog,
                version_changes.metric_attribute_changes(),
            )?
            .into_iter()
            .for_each(|attr| {
                metric_group_attrs.insert(attr.id(), attr);
            });

            // Process each metric defined in the metric group.
            let mut all_shared_attributes = vec![];
            let mut required_shared_attributes = HashSet::new();
            for (i, metric) in metrics.metrics.iter_mut().enumerate() {
                if let Metric::Ref { r#ref, tags } = metric {
                    if let Some(referenced_metric) = sem_conv_catalog.metric(r#ref) {
                        let inherited_attrs = referenced_metric.attributes.clone();

                        // Initialize all/required_shared_attributes only if first metric.
                        if i == 0 {
                            all_shared_attributes = inherited_attrs.clone();
                            all_shared_attributes
                                .iter()
                                .filter(|attr| attr.is_required())
                                .for_each(|attr| {
                                    required_shared_attributes.insert(attr.id());
                                });
                        }

                        let mut required_count = 0;
                        for attr in inherited_attrs.iter() {
                            if attr.is_required() {
                                required_count += 1;
                                if !required_shared_attributes.contains(&attr.id()) {
                                    return Err(Error::IncompatibleMetricAttributes {
                                        metric_group_ref: metrics.name.clone(),
                                        metric_ref: referenced_metric.name.clone(),
                                        error: format!("The attribute '{}' is required but not required in other metrics", attr.id()),
                                    });
                                }
                            }
                        }
                        if required_count != required_shared_attributes.len() {
                            return Err(Error::IncompatibleMetricAttributes {
                                metric_group_ref: metrics.name.clone(),
                                metric_ref: referenced_metric.name.clone(),
                                error: "Some required attributes are missing in this metric"
                                    .to_string(),
                            });
                        }

                        *metric = Metric::Metric {
                            name: referenced_metric.name.clone(),
                            brief: referenced_metric.brief.clone(),
                            note: referenced_metric.note.clone(),
                            attributes: vec![],
                            instrument: referenced_metric.instrument.clone(),
                            unit: referenced_metric.unit.clone(),
                            tags: tags.clone(),
                        };
                    } else {
                        return Err(Error::FailToResolveMetric {
                            r#ref: r#ref.clone(),
                        });
                    }
                }
            }

            let all_shared_attributes = resolve_attributes(
                &to_schema_attributes(&all_shared_attributes),
                sem_conv_catalog,
                version_changes.metric_attribute_changes(),
            )?;
            all_shared_attributes
                .into_iter()
                .for_each(|attr| _ = metric_group_attrs.insert(attr.id(), attr));

            metrics.attributes = metric_group_attrs.into_values().collect();
        }
    }
    Ok(())
}

/// Converts a semantic convention metric to a resolved metric that will be
/// part of the catalog of metrics of a resolved telemetry schema.
///
/// Note: References to attribute of the metric are not part of the catalog of
/// metrics but are part of the schema specification in the instrumentation
/// library section.
pub fn semconv_to_resolved_metric(
    metric: &weaver_semconv::metric::MetricSpec,
) -> weaver_resolved_schema::metric::Metric {
    weaver_resolved_schema::metric::Metric {
        name: metric.name.clone(),
        brief: metric.brief.clone(),
        note: metric.note.clone(),
        instrument: resolve_instrument(&metric.instrument),
        unit: metric.unit.clone(),
        tags: None, // ToDo we need a mechanism to transmit tags here from the input schema.
    }
}

/// Resolve a metric instrument.
pub fn resolve_instrument(
    instrument: &InstrumentSpec,
) -> weaver_resolved_schema::metric::Instrument {
    match instrument {
        InstrumentSpec::Counter => weaver_resolved_schema::metric::Instrument::Counter,
        InstrumentSpec::UpDownCounter => weaver_resolved_schema::metric::Instrument::UpDownCounter,
        InstrumentSpec::Gauge => weaver_resolved_schema::metric::Instrument::Gauge,
        InstrumentSpec::Histogram => weaver_resolved_schema::metric::Instrument::Histogram,
    }
}
