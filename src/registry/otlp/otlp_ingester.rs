use std::time::Duration;

use log::info;
use serde_json::{json, Value};
use weaver_common::log_info;
use weaver_live_check::{
    sample_attribute::SampleAttribute,
    sample_resource::SampleResource,
    sample_span::{SampleSpan, SampleSpanEvent, SampleSpanLink},
    Error, Ingester, Sample,
};
use weaver_semconv::group::SpanKindSpec;

use super::{
    grpc_stubs::proto::common::v1::{AnyValue, KeyValue},
    listen_otlp_requests, OtlpRequest,
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

    fn maybe_to_json(value: Option<AnyValue>) -> Option<Value> {
        if let Some(value) = value {
            if let Some(value) = value.value {
                use crate::registry::otlp::grpc_stubs::proto::common::v1::any_value::Value as GrpcValue;
                match value {
                    GrpcValue::StringValue(string) => Some(Value::String(string)),
                    GrpcValue::IntValue(int_value) => Some(Value::Number(int_value.into())),
                    GrpcValue::DoubleValue(double_value) => Some(json!(double_value)),
                    GrpcValue::BoolValue(bool_value) => Some(Value::Bool(bool_value)),
                    GrpcValue::ArrayValue(array_value) => {
                        let mut vec = Vec::new();
                        for value in array_value.values {
                            if let Some(value) = Self::maybe_to_json(Some(value)) {
                                vec.push(value);
                            }
                        }
                        Some(Value::Array(vec))
                    }
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    // TODO Ideally this would be a TryFrom in the SampleAttribute but requires
    // the grpc_stubs to be in another crate
    fn sample_attribute_from_key_value(key_value: &KeyValue) -> SampleAttribute {
        let value = Self::maybe_to_json(key_value.value.clone());
        let r#type = match value {
            Some(ref val) => SampleAttribute::infer_type(val),
            None => None,
        };
        SampleAttribute {
            name: key_value.key.clone(),
            value,
            r#type,
            live_check_result: None,
        }
    }

    fn span_kind_from_otlp_kind(kind: i32) -> SpanKindSpec {
        match kind {
            2 => SpanKindSpec::Server,
            3 => SpanKindSpec::Client,
            4 => SpanKindSpec::Producer,
            5 => SpanKindSpec::Consumer,
            _ => SpanKindSpec::Internal,
        }
    }

    fn fill_buffer_from_request(&mut self, request: OtlpRequest) -> Option<usize> {
        match request {
            OtlpRequest::Logs(_logs) => {
                // TODO Implement the checking logic for logs
                Some(0)
            }
            OtlpRequest::Metrics(_metrics) => {
                // TODO Implement the checking logic for metrics
                Some(0)
            }
            OtlpRequest::Traces(trace) => {
                for resource_span in trace.resource_spans {
                    if let Some(resource) = resource_span.resource {
                        let mut sample_resource = SampleResource {
                            attributes: Vec::new(),
                            live_check_result: None,
                        };
                        for attribute in resource.attributes {
                            sample_resource
                                .attributes
                                .push(Self::sample_attribute_from_key_value(&attribute));
                        }
                        self.buffer.push(Sample::Resource(sample_resource));
                    }

                    for scope_span in resource_span.scope_spans {
                        if let Some(scope) = scope_span.scope {
                            // TODO SampleInstrumentationScope?
                            for attribute in scope.attributes {
                                self.buffer.push(Sample::Attribute(
                                    Self::sample_attribute_from_key_value(&attribute),
                                ));
                            }
                        }

                        for span in scope_span.spans {
                            let mut sample_span = SampleSpan {
                                name: span.name,
                                kind: Self::span_kind_from_otlp_kind(span.kind),
                                attributes: Vec::new(),
                                span_events: Vec::new(),
                                span_links: Vec::new(),
                                live_check_result: None,
                            };
                            for attribute in span.attributes {
                                sample_span
                                    .attributes
                                    .push(Self::sample_attribute_from_key_value(&attribute));
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
                                        .push(Self::sample_attribute_from_key_value(&attribute));
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
                                        .push(Self::sample_attribute_from_key_value(&attribute));
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

impl Ingester for OtlpIngester {
    fn ingest(&self) -> Result<Box<dyn Iterator<Item = Sample>>, Error> {
        let otlp_requests = listen_otlp_requests(
            self.otlp_grpc_address.as_str(),
            self.otlp_grpc_port,
            self.admin_port,
            Duration::from_secs(self.inactivity_timeout),
        )
        .map_err(|e| Error::IngestError {
            error: format!("Failed to listen to OTLP requests: {}", e),
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
        info!(
            "The OTLP receiver will stop after {} seconds of inactivity.",
            self.inactivity_timeout
        );

        Ok(Box::new(OtlpIterator::new(Box::new(otlp_requests))))
    }
}
