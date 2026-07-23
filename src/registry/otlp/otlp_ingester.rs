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
        otlp_log_record_to_sample_log, otlp_metric_to_sample, sample_attribute_from_key_value,
        span_kind_from_otlp_kind, status_from_otlp_status,
    },
    listen_otlp_requests, AdminController, AdminDrainGuard, OtlpRequest,
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
                        if let Some(scope) = scope_log.scope {
                            for attribute in scope.attributes {
                                self.buffer.push(Sample::Attribute(
                                    sample_attribute_from_key_value(&attribute),
                                ));
                            }
                        }

                        for log_record in scope_log.log_records {
                            let mut sample_log = otlp_log_record_to_sample_log(&log_record);
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
                        if let Some(scope) = scope_metric.scope {
                            // TODO SampleInstrumentationScope?
                            for attribute in scope.attributes {
                                self.buffer.push(Sample::Attribute(
                                    sample_attribute_from_key_value(&attribute),
                                ));
                            }
                        }

                        for metric in scope_metric.metrics {
                            let mut sample_metric = otlp_metric_to_sample(metric);
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
                        if let Some(scope) = scope_span.scope {
                            // TODO SampleInstrumentationScope?
                            for attribute in scope.attributes {
                                self.buffer.push(Sample::Attribute(
                                    sample_attribute_from_key_value(&attribute),
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
    /// Ingest OTLP data and return the sample iterator, the admin controller,
    /// and the OTLP receiver's background-thread join handle.
    ///
    /// The `AdminController` can be used to send a formatted report back through
    /// the `/stop` HTTP endpoint when `--output http` is used. The join handle
    /// only resolves once the gRPC server, HTTP admin server, and background
    /// tasks have gracefully shut down — callers should join it before
    /// exiting the process to avoid cutting off an in-flight request.
    pub fn ingest_otlp(
        &self,
    ) -> Result<
        (
            Box<dyn Iterator<Item = Sample>>,
            AdminController,
            std::thread::JoinHandle<()>,
        ),
        Error,
    > {
        let (otlp_requests, controller, handle) = listen_otlp_requests(
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
            controller,
            handle,
        ))
    }
}

/// Wraps the OTLP sample iterator so that, consumed through the generic
/// [`Ingester::ingest`] trait method, the admin thread still gets signalled
/// to shut down and drained once iteration ends.
///
/// The generic trait has no way to expose the `AdminController`/`JoinHandle`
/// that [`OtlpIngester::ingest_otlp`] returns, so without this, dropping
/// them here would silently detach the background thread — reintroducing
/// the stop-race this module's shutdown coordination exists to prevent.
/// `_guard`'s own `Drop` impl does the actual signal-and-wait; it's never
/// read otherwise, only dropped alongside `inner`.
struct DrainOnDrop {
    inner: Box<dyn Iterator<Item = Sample>>,
    _guard: AdminDrainGuard,
}

impl Iterator for DrainOnDrop {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        self.inner.next()
    }
}

impl Ingester for OtlpIngester {
    fn ingest(&self) -> Result<Box<dyn Iterator<Item = Sample>>, Error> {
        let (iterator, controller, handle) = self.ingest_otlp()?;
        Ok(Box::new(DrainOnDrop {
            inner: iterator,
            _guard: AdminDrainGuard::new(controller, handle),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_test_support::reserve_test_port;

    #[test]
    fn generic_ingest_drains_admin_thread_on_early_drop() {
        let grpc_port = reserve_test_port();
        let admin_port = reserve_test_port();
        let ingester = OtlpIngester {
            otlp_grpc_address: "127.0.0.1".to_owned(),
            otlp_grpc_port: grpc_port,
            admin_port,
            // Disabled: nothing internal should ever request shutdown here.
            inactivity_timeout: 0,
        };

        // Go through the generic `Ingester::ingest()` trait method — the
        // entry point that has no direct access to the AdminController or
        // JoinHandle `ingest_otlp` returns.
        let iterator = ingester.ingest().expect("Failed to start OTLP ingester");

        // Give the server a little time to finish binding the port.
        std::thread::sleep(Duration::from_millis(200));

        // Drop the iterator without ever consuming a sample or sending any
        // stop signal (no /stop, no CTRL+C, no inactivity) — simulating a
        // caller that simply stops reading. The admin thread must still be
        // signalled and drained: the admin port should no longer be
        // accepting connections.
        drop(iterator);

        assert!(
            std::net::TcpStream::connect(("127.0.0.1", admin_port)).is_err(),
            "Admin port is still accepting connections after the generic ingest() \
             iterator was dropped early — the background thread was not drained"
        );
    }
}
