use std::time::Duration;

use serde_json::{json, Value};
use weaver_common::Logger;
use weaver_health::{sample::SampleAttribute, Error, Ingester};

use super::{grpc_stubs::proto::common::v1::AnyValue, listen_otlp_requests, OtlpRequest};

/// An ingester for OTLP data
pub struct AttributeOtlpIngester {
    /// The address of the OTLP gRPC server
    pub otlp_grpc_address: String,
    /// The port of the OTLP gRPC server
    pub otlp_grpc_port: u16,
    /// The port of the admin server
    pub admin_port: u16,
    /// The inactivity timeout
    pub inactivity_timeout: u64,
}

impl AttributeOtlpIngester {
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
}

/// Iterator for OTLP attributes
struct OtlpAttributeIterator {
    otlp_requests: Box<dyn Iterator<Item = OtlpRequest>>,
    buffer: Vec<SampleAttribute>,
}

impl OtlpAttributeIterator {
    fn new(otlp_requests: Box<dyn Iterator<Item = OtlpRequest>>) -> Self {
        Self {
            otlp_requests,
            buffer: Vec::new(),
        }
    }

    fn extract_attributes_from_request(&mut self, request: OtlpRequest) -> Option<SampleAttribute> {
        match request {
            OtlpRequest::Logs(_logs) => {
                // TODO Implement the checking logic for logs
                // self.logger.error("Logs Request received");
                self.next()
            }
            OtlpRequest::Metrics(_metrics) => {
                // TODO Implement the checking logic for metrics
                // self.logger.error("Metrics Request received");
                self.next()
            }
            OtlpRequest::Traces(trace) => {
                // Process and buffer all attributes from the trace
                for resource_span in trace.resource_spans {
                    if let Some(resource) = resource_span.resource {
                        for attribute in resource.attributes {
                            self.buffer.push(SampleAttribute {
                                name: attribute.key,
                                value: AttributeOtlpIngester::maybe_to_json(attribute.value),
                                r#type: None,
                            });
                        }
                    }

                    for scope_span in resource_span.scope_spans {
                        if let Some(scope) = scope_span.scope {
                            for attribute in scope.attributes {
                                self.buffer.push(SampleAttribute {
                                    name: attribute.key,
                                    value: AttributeOtlpIngester::maybe_to_json(attribute.value),
                                    r#type: None,
                                });
                            }
                        }

                        for span in scope_span.spans {
                            for attribute in span.attributes {
                                self.buffer.push(SampleAttribute {
                                    name: attribute.key,
                                    value: AttributeOtlpIngester::maybe_to_json(attribute.value),
                                    r#type: None,
                                });
                            }
                            for event in span.events {
                                for attribute in event.attributes {
                                    self.buffer.push(SampleAttribute {
                                        name: attribute.key,
                                        value: AttributeOtlpIngester::maybe_to_json(
                                            attribute.value,
                                        ),
                                        r#type: None,
                                    });
                                }
                            }
                            for link in span.links {
                                for attribute in link.attributes {
                                    self.buffer.push(SampleAttribute {
                                        name: attribute.key,
                                        value: AttributeOtlpIngester::maybe_to_json(
                                            attribute.value,
                                        ),
                                        r#type: None,
                                    });
                                }
                            }
                        }
                    }
                }

                // Return the first buffered attribute if available
                if !self.buffer.is_empty() {
                    Some(self.buffer.remove(0))
                } else {
                    self.next()
                }
            }
            OtlpRequest::Stop(_reason) => {
                // self.logger
                //     .warn(&format!("Stopping the listener, reason: {}", reason));
                None
            }
            OtlpRequest::Error(_error) => {
                // self.logger
                //     .error(&format!("Error in OTLP request: {}", error));
                None
            }
        }
    }
}

impl Iterator for OtlpAttributeIterator {
    type Item = SampleAttribute;

    fn next(&mut self) -> Option<Self::Item> {
        // First check if we have buffered items
        if !self.buffer.is_empty() {
            return Some(self.buffer.remove(0));
        }

        // Otherwise process the next OTLP request
        match self.otlp_requests.next() {
            Some(request) => self.extract_attributes_from_request(request),
            None => None,
        }
    }
}

impl Ingester<SampleAttribute> for AttributeOtlpIngester {
    fn ingest(
        &self,
        logger: impl Logger + Sync + Clone + 'static,
    ) -> Result<Box<dyn Iterator<Item = SampleAttribute>>, Error> {
        let otlp_requests = listen_otlp_requests(
            self.otlp_grpc_address.as_str(),
            self.otlp_grpc_port,
            self.admin_port,
            Duration::from_secs(self.inactivity_timeout),
            logger.clone(),
        )
        .map_err(|e| Error::IngestError {
            error: format!("Failed to listen to OTLP requests: {}", e),
        })?;

        Ok(Box::new(OtlpAttributeIterator::new(Box::new(
            otlp_requests,
        ))))
    }
}
