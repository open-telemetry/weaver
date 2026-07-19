// SPDX-License-Identifier: Apache-2.0

//! OTLP ingester
use std::rc::Rc;
use std::time::Duration;

use log::info;
use weaver_common::log_info;
use weaver_live_check::{
    sample_resource::SampleResource,
    sample_span::{SampleSpan, SampleSpanEvent, SampleSpanLink},
    Error, Ingester, Sample,
};

use super::{
    conversion::{
        otlp_instrumentation_scope_to_sample, otlp_log_record_to_sample_log, otlp_metric_to_sample,
        sample_attribute_from_key_value, span_kind_from_otlp_kind, status_from_otlp_status,
    },
    listen_otlp_requests, AdminReportSender, OtlpRequest,
};

/// An ingester for OTLP data
pub struct OtlpIngester {
    /// The address of the OTLP gRPC server
    pub otlp_grpc_address: String,
    /// The port of the OTLP gRPC server
    pub otlp_grpc_port: u16,
    /// The port of the admin server
    pub admin_port: u16,
    /// The inactivity timeout
    pub inactivity_timeout: u64,
}

/// Iterator for OTLP samples
struct OtlpIterator {
    otlp_requests: Box<dyn Iterator<Item = OtlpRequest>>,
    buffer: Vec<Sample>,
}

impl OtlpIterator {
    fn new(otlp_requests: Box<dyn Iterator<Item = OtlpRequest>>) -> Self {
        Self {
            otlp_requests,
            buffer: Vec::new(),
        }
    }

    fn fill_buffer_from_request(&mut self, request: OtlpRequest) -> Option<usize> {
        match request {
            OtlpRequest::Logs(logs) => {
                for resource_log in logs.resource_logs {
                    let rc_resource = if let Some(resource) = resource_log.resource {
                        let mut sample_resource = SampleResource {
                            attributes: Vec::new(),
                            live_check_result: None,
                        };
                        for attribute in resource.attributes {
                            sample_resource
                                .attributes
                                .push(sample_attribute_from_key_value(&attribute));
                        }
                        let rc = Rc::new(sample_resource);
                        self.buffer.push(Sample::Resource((*rc).clone()));
                        Some(rc)
                    } else {
                        None
                    };

                    for scope_log in resource_log.scope_logs {
                        let instrumentation_scope = otlp_instrumentation_scope_to_sample(
                            scope_log.scope.as_ref(),
                            &scope_log.schema_url,
                        );
                        if let Some(scope) = scope_log.scope.as_ref() {
                            for attribute in &scope.attributes {
                                self.buffer.push(Sample::Attribute(
                                    sample_attribute_from_key_value(attribute),
                                ));
                            }
                        }

                        for log_record in scope_log.log_records {
                            let mut sample_log = otlp_log_record_to_sample_log(&log_record);
                            sample_log.instrumentation_scope = instrumentation_scope.clone();
                            sample_log.resource = rc_resource.clone();
                            self.buffer.push(Sample::Log(sample_log));
                        }
                    }
                }
                Some(self.buffer.len())
            }
            OtlpRequest::Metrics(metrics) => {
                for resource_metric in metrics.resource_metrics {
                    let rc_resource = if let Some(resource) = resource_metric.resource {
                        let mut sample_resource = SampleResource {
                            attributes: Vec::new(),
                            live_check_result: None,
                        };
                        for attribute in resource.attributes {
                            sample_resource
                                .attributes
                                .push(sample_attribute_from_key_value(&attribute));
                        }
                        let rc = Rc::new(sample_resource);
                        self.buffer.push(Sample::Resource((*rc).clone()));
                        Some(rc)
                    } else {
                        None
                    };

                    for scope_metric in resource_metric.scope_metrics {
                        let instrumentation_scope = otlp_instrumentation_scope_to_sample(
                            scope_metric.scope.as_ref(),
                            &scope_metric.schema_url,
                        );
                        if let Some(scope) = scope_metric.scope.as_ref() {
                            for attribute in &scope.attributes {
                                self.buffer.push(Sample::Attribute(
                                    sample_attribute_from_key_value(attribute),
                                ));
                            }
                        }

                        for metric in scope_metric.metrics {
                            let mut sample_metric = otlp_metric_to_sample(metric);
                            sample_metric.instrumentation_scope = instrumentation_scope.clone();
                            sample_metric.resource = rc_resource.clone();
                            self.buffer.push(Sample::Metric(sample_metric));
                        }
                    }
                }
                Some(self.buffer.len())
            }
            OtlpRequest::Traces(trace) => {
                for resource_span in trace.resource_spans {
                    let rc_resource = if let Some(resource) = resource_span.resource {
                        let mut sample_resource = SampleResource {
                            attributes: Vec::new(),
                            live_check_result: None,
                        };
                        for attribute in resource.attributes {
                            sample_resource
                                .attributes
                                .push(sample_attribute_from_key_value(&attribute));
                        }
                        let rc = Rc::new(sample_resource);
                        self.buffer.push(Sample::Resource((*rc).clone()));
                        Some(rc)
                    } else {
                        None
                    };

                    for scope_span in resource_span.scope_spans {
                        let instrumentation_scope = otlp_instrumentation_scope_to_sample(
                            scope_span.scope.as_ref(),
                            &scope_span.schema_url,
                        );
                        if let Some(scope) = scope_span.scope.as_ref() {
                            for attribute in &scope.attributes {
                                self.buffer.push(Sample::Attribute(
                                    sample_attribute_from_key_value(attribute),
                                ));
                            }
                        }

                        for span in scope_span.spans {
                            let span_kind = span.kind();
                            let mut sample_span = SampleSpan {
                                name: span.name,
                                kind: span_kind_from_otlp_kind(span_kind),
                                status: status_from_otlp_status(span.status),
                                attributes: Vec::new(),
                                span_events: Vec::new(),
                                span_links: Vec::new(),
                                instrumentation_scope: instrumentation_scope.clone(),
                                live_check_result: None,
                                resource: rc_resource.clone(),
                            };
                            for attribute in span.attributes {
                                sample_span
                                    .attributes
                                    .push(sample_attribute_from_key_value(&attribute));
                            }
                            for event in span.events {
                                let mut sample_event = SampleSpanEvent {
                                    name: event.name,
                                    attributes: Vec::new(),
                                    live_check_result: None,
                                };
                                for attribute in event.attributes {
                                    sample_event
                                        .attributes
                                        .push(sample_attribute_from_key_value(&attribute));
                                }
                                sample_span.span_events.push(sample_event);
                            }
                            for link in span.links {
                                let mut sample_link = SampleSpanLink {
                                    attributes: Vec::new(),
                                    live_check_result: None,
                                };
                                for attribute in link.attributes {
                                    sample_link
                                        .attributes
                                        .push(sample_attribute_from_key_value(&attribute));
                                }
                                sample_span.span_links.push(sample_link);
                            }
                            self.buffer.push(Sample::Span(sample_span));
                        }
                    }
                }
                Some(self.buffer.len())
            }
            OtlpRequest::Stop(_reason) => None,
            OtlpRequest::Error(_error) => None,
        }
    }
}

impl Iterator for OtlpIterator {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        while self.buffer.is_empty() {
            match self.otlp_requests.next() {
                Some(request) => {
                    let _bufsize = self.fill_buffer_from_request(request)?;
                }
                None => return None,
            }
        }

        Some(self.buffer.remove(0))
    }
}

impl OtlpIngester {
    /// Ingest OTLP data and return both the sample iterator and the admin report sender.
    ///
    /// The `AdminReportSender` can be used to send a formatted report back through
    /// the `/stop` HTTP endpoint when `--output http` is used.
    pub fn ingest_otlp(
        &self,
    ) -> Result<(Box<dyn Iterator<Item = Sample>>, AdminReportSender), Error> {
        let (otlp_requests, report_sender) = listen_otlp_requests(
            self.otlp_grpc_address.as_str(),
            self.otlp_grpc_port,
            self.admin_port,
            Duration::from_secs(self.inactivity_timeout),
        )
        .map_err(|e| Error::IngestError {
            error: format!("Failed to listen to OTLP requests: {e}"),
        })?;

        log_info("To stop the OTLP receiver:");
        info!("  - press CTRL+C,");
        info!(
            "  - send a SIGHUP signal to the weaver process or run this command kill -SIGHUP {}",
            std::process::id()
        );
        info!(
            "  - or send a POST request to the /stop endpoint via the following command curl -X POST http://localhost:{}/stop.",
            self.admin_port
        );
        if self.inactivity_timeout == 0 {
            info!("The OTLP receiver will run indefinitely until stopped manually.");
        } else {
            info!(
                "The OTLP receiver will stop after {} seconds of inactivity.",
                self.inactivity_timeout
            );
        };

        Ok((
            Box::new(OtlpIterator::new(Box::new(otlp_requests))),
            report_sender,
        ))
    }
}

impl Ingester for OtlpIngester {
    fn ingest(&self) -> Result<Box<dyn Iterator<Item = Sample>>, Error> {
        let (iterator, _report_sender) = self.ingest_otlp()?;
        Ok(iterator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::otlp::grpc_stubs::proto::{
        collector::{
            logs::v1::ExportLogsServiceRequest, metrics::v1::ExportMetricsServiceRequest,
            trace::v1::ExportTraceServiceRequest,
        },
        common::v1::{any_value, AnyValue, InstrumentationScope, KeyValue},
        logs::v1::{LogRecord, ResourceLogs, ScopeLogs},
        metrics::v1::{Metric, ResourceMetrics, ScopeMetrics},
        resource::v1::Resource,
        trace::v1::{ResourceSpans, ScopeSpans, Span},
    };

    fn string_attribute(name: &str, value: &str) -> KeyValue {
        KeyValue {
            key: name.to_owned(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(value.to_owned())),
            }),
        }
    }

    fn scope(name: &str) -> InstrumentationScope {
        InstrumentationScope {
            name: name.to_owned(),
            version: "1.2.3".to_owned(),
            attributes: vec![string_attribute("scope.environment", "test")],
            dropped_attributes_count: 2,
        }
    }

    fn collect(requests: Vec<OtlpRequest>) -> Vec<Sample> {
        OtlpIterator::new(Box::new(requests.into_iter())).collect()
    }

    #[test]
    fn same_named_spans_keep_distinct_instrumentation_scopes() {
        let request = OtlpRequest::Traces(ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                scope_spans: vec![
                    ScopeSpans {
                        scope: Some(scope("library-a")),
                        spans: vec![Span {
                            name: "shared-operation".to_owned(),
                            ..Default::default()
                        }],
                        schema_url: "https://example.test/schema/a".to_owned(),
                    },
                    ScopeSpans {
                        scope: Some(scope("library-b")),
                        spans: vec![Span {
                            name: "shared-operation".to_owned(),
                            ..Default::default()
                        }],
                        schema_url: "https://example.test/schema/b".to_owned(),
                    },
                ],
                ..Default::default()
            }],
        });

        let scopes: Vec<_> = collect(vec![request])
            .into_iter()
            .filter_map(|sample| match sample {
                Sample::Span(span) => span.instrumentation_scope,
                _ => None,
            })
            .collect();

        assert_eq!(scopes.len(), 2);
        assert_eq!(scopes[0].name, "library-a");
        assert_eq!(scopes[0].schema_url, "https://example.test/schema/a");
        assert_eq!(scopes[1].name, "library-b");
        assert_eq!(scopes[1].schema_url, "https://example.test/schema/b");
    }

    #[test]
    fn instrumentation_scope_reaches_spans_metrics_and_logs() {
        let requests = vec![
            OtlpRequest::Traces(ExportTraceServiceRequest {
                resource_spans: vec![ResourceSpans {
                    scope_spans: vec![ScopeSpans {
                        scope: Some(scope("trace-library")),
                        spans: vec![Span {
                            name: "operation".to_owned(),
                            ..Default::default()
                        }],
                        schema_url: "https://example.test/trace".to_owned(),
                    }],
                    ..Default::default()
                }],
            }),
            OtlpRequest::Metrics(ExportMetricsServiceRequest {
                resource_metrics: vec![ResourceMetrics {
                    scope_metrics: vec![ScopeMetrics {
                        scope: Some(scope("metric-library")),
                        metrics: vec![Metric {
                            name: "requests".to_owned(),
                            ..Default::default()
                        }],
                        schema_url: "https://example.test/metric".to_owned(),
                    }],
                    ..Default::default()
                }],
            }),
            OtlpRequest::Logs(ExportLogsServiceRequest {
                resource_logs: vec![ResourceLogs {
                    scope_logs: vec![ScopeLogs {
                        scope: Some(scope("log-library")),
                        log_records: vec![LogRecord {
                            event_name: "request.completed".to_owned(),
                            ..Default::default()
                        }],
                        schema_url: "https://example.test/log".to_owned(),
                    }],
                    ..Default::default()
                }],
            }),
        ];

        let samples = collect(requests);
        let span_scope = samples.iter().find_map(|sample| match sample {
            Sample::Span(span) => span.instrumentation_scope.as_ref(),
            _ => None,
        });
        let metric_scope = samples.iter().find_map(|sample| match sample {
            Sample::Metric(metric) => metric.instrumentation_scope.as_ref(),
            _ => None,
        });
        let log_scope = samples.iter().find_map(|sample| match sample {
            Sample::Log(log) => log.instrumentation_scope.as_ref(),
            _ => None,
        });

        assert_eq!(span_scope.expect("span scope").name, "trace-library");
        assert_eq!(metric_scope.expect("metric scope").name, "metric-library");
        assert_eq!(log_scope.expect("log scope").name, "log-library");
    }

    #[test]
    fn missing_scope_stays_absent_but_schema_only_scope_is_preserved() {
        let request = OtlpRequest::Traces(ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                scope_spans: vec![
                    ScopeSpans {
                        scope: None,
                        spans: vec![Span {
                            name: "unknown-owner".to_owned(),
                            ..Default::default()
                        }],
                        schema_url: String::new(),
                    },
                    ScopeSpans {
                        scope: None,
                        spans: vec![Span {
                            name: "schema-owned".to_owned(),
                            ..Default::default()
                        }],
                        schema_url: "https://example.test/schema-only".to_owned(),
                    },
                ],
                ..Default::default()
            }],
        });

        let spans: Vec<_> = collect(vec![request])
            .into_iter()
            .filter_map(|sample| match sample {
                Sample::Span(span) => Some(span),
                _ => None,
            })
            .collect();

        assert!(spans[0].instrumentation_scope.is_none());
        let schema_only = spans[1]
            .instrumentation_scope
            .as_ref()
            .expect("schema URL is ownership metadata even when scope is absent");
        assert_eq!(schema_only.name, "");
        assert_eq!(schema_only.schema_url, "https://example.test/schema-only");
    }

    #[test]
    fn scope_attributes_are_attached_and_emitted_for_checking_exactly_once() {
        let request = OtlpRequest::Traces(ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource {
                    attributes: vec![string_attribute("service.name", "checkout")],
                    ..Default::default()
                }),
                scope_spans: vec![ScopeSpans {
                    scope: Some(scope("trace-library")),
                    spans: vec![Span {
                        name: "operation".to_owned(),
                        ..Default::default()
                    }],
                    schema_url: "https://example.test/trace".to_owned(),
                }],
                ..Default::default()
            }],
        });

        let samples = collect(vec![request]);
        let checked_scope_attributes = samples
            .iter()
            .filter(|sample| {
                matches!(sample, Sample::Attribute(attribute) if attribute.name == "scope.environment")
            })
            .count();
        assert_eq!(checked_scope_attributes, 1);

        let span = samples
            .iter()
            .find_map(|sample| match sample {
                Sample::Span(span) => Some(span),
                _ => None,
            })
            .expect("span sample");
        assert_eq!(
            span.resource.as_ref().expect("resource").attributes[0].name,
            "service.name"
        );
        assert_eq!(
            span.instrumentation_scope
                .as_ref()
                .expect("scope")
                .attributes[0]
                .name,
            "scope.environment"
        );
    }
}
