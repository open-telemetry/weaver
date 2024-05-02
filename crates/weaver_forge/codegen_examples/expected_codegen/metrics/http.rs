/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/metrics/metrics.rs.j2

use crate::metrics::{HistogramProvider, UpDownCounterProvider};


/// Duration of HTTP server requests.
pub fn create_http_server_request_duration<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<T>
    where opentelemetry::metrics::Meter: HistogramProvider<T> {
    meter.create_histogram("http.server.request.duration", "Duration of HTTP server requests.", "s")
}

/// Metric: http.server.request.duration
/// Brief: Duration of HTTP server requests.
/// Unit: s
#[derive(Debug)]
pub struct HttpServerRequestDuration<T>(opentelemetry::metrics::Histogram<T>);

/// Attributes for the `http.server.request.duration` metric.
#[derive(Debug, Clone)]
pub struct HttpServerRequestDurationAttributes {
    /// HTTP request method.
    pub http_request_method: crate::attributes::http::HttpRequestMethod,
    /// [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6).
    pub http_response_status_code: Option<i64>,
    /// The matched route, that is, the path template in the format used by the respective server framework.
    pub http_route: Option<String>,
    /// Describes a class of error the operation ended with.
    pub error_type: Option<crate::attributes::error::ErrorType>,
    /// [OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.
    pub network_protocol_name: Option<String>,
    /// The actual version of the protocol used for network communication.
    pub network_protocol_version: Option<String>,
    /// Name of the local HTTP server that received the request.
    pub server_address: Option<String>,
    /// Port of the local HTTP server that received the request.
    pub server_port: Option<i64>,
    /// The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.
    pub url_scheme: String,
    
}

impl <T> HttpServerRequestDuration<T> {
    pub fn new(meter: &opentelemetry::metrics::Meter) -> Self
        where opentelemetry::metrics::Meter: HistogramProvider<T>{
        Self(meter.create_histogram("http.server.request.duration", "Duration of HTTP server requests.", "s"))
    }

    /// Adds an additional value to the distribution.
    pub fn record(&self, value: T, attributes: HttpServerRequestDurationAttributes) {
        // self.0.record(value, attributes.into())
    }
}

/// Number of active HTTP server requests.
#[cfg(feature = "semconv_experimental")]
pub fn create_http_server_active_requests<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::UpDownCounter<T>
    where opentelemetry::metrics::Meter: UpDownCounterProvider<T> {
    meter.create_up_down_counter("http.server.active_requests", "Number of active HTTP server requests.", "{request}")
}

/// Size of HTTP server request bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_http_server_request_body_size<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<T>
    where opentelemetry::metrics::Meter: HistogramProvider<T> {
    meter.create_histogram("http.server.request.body.size", "Size of HTTP server request bodies.", "By")
}

/// Metric: http.server.request.body.size
/// Brief: Size of HTTP server request bodies.
/// Unit: By
#[derive(Debug)]
pub struct HttpServerRequestBodySize<T>(opentelemetry::metrics::Histogram<T>);

/// Attributes for the `http.server.request.body.size` metric.
#[derive(Debug, Clone)]
pub struct HttpServerRequestBodySizeAttributes {
    /// HTTP request method.
    pub http_request_method: crate::attributes::http::HttpRequestMethod,
    /// [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6).
    pub http_response_status_code: Option<i64>,
    /// The matched route, that is, the path template in the format used by the respective server framework.
    pub http_route: Option<String>,
    /// Describes a class of error the operation ended with.
    pub error_type: Option<crate::attributes::error::ErrorType>,
    /// [OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.
    pub network_protocol_name: Option<String>,
    /// The actual version of the protocol used for network communication.
    pub network_protocol_version: Option<String>,
    /// Name of the local HTTP server that received the request.
    pub server_address: Option<String>,
    /// Port of the local HTTP server that received the request.
    pub server_port: Option<i64>,
    /// The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.
    pub url_scheme: String,
    
}

impl <T> HttpServerRequestBodySize<T> {
    pub fn new(meter: &opentelemetry::metrics::Meter) -> Self
        where opentelemetry::metrics::Meter: HistogramProvider<T>{
        Self(meter.create_histogram("http.server.request.body.size", "Size of HTTP server request bodies.", "By"))
    }

    /// Adds an additional value to the distribution.
    pub fn record(&self, value: T, attributes: HttpServerRequestBodySizeAttributes) {
        // self.0.record(value, attributes.into())
    }
}

/// Size of HTTP server response bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_http_server_response_body_size<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<T>
    where opentelemetry::metrics::Meter: HistogramProvider<T> {
    meter.create_histogram("http.server.response.body.size", "Size of HTTP server response bodies.", "By")
}

/// Metric: http.server.response.body.size
/// Brief: Size of HTTP server response bodies.
/// Unit: By
#[derive(Debug)]
pub struct HttpServerResponseBodySize<T>(opentelemetry::metrics::Histogram<T>);

/// Attributes for the `http.server.response.body.size` metric.
#[derive(Debug, Clone)]
pub struct HttpServerResponseBodySizeAttributes {
    /// HTTP request method.
    pub http_request_method: crate::attributes::http::HttpRequestMethod,
    /// [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6).
    pub http_response_status_code: Option<i64>,
    /// The matched route, that is, the path template in the format used by the respective server framework.
    pub http_route: Option<String>,
    /// Describes a class of error the operation ended with.
    pub error_type: Option<crate::attributes::error::ErrorType>,
    /// [OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.
    pub network_protocol_name: Option<String>,
    /// The actual version of the protocol used for network communication.
    pub network_protocol_version: Option<String>,
    /// Name of the local HTTP server that received the request.
    pub server_address: Option<String>,
    /// Port of the local HTTP server that received the request.
    pub server_port: Option<i64>,
    /// The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.
    pub url_scheme: String,
    
}

impl <T> HttpServerResponseBodySize<T> {
    pub fn new(meter: &opentelemetry::metrics::Meter) -> Self
        where opentelemetry::metrics::Meter: HistogramProvider<T>{
        Self(meter.create_histogram("http.server.response.body.size", "Size of HTTP server response bodies.", "By"))
    }

    /// Adds an additional value to the distribution.
    pub fn record(&self, value: T, attributes: HttpServerResponseBodySizeAttributes) {
        // self.0.record(value, attributes.into())
    }
}

/// Duration of HTTP client requests.
pub fn create_http_client_request_duration<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<T>
    where opentelemetry::metrics::Meter: HistogramProvider<T> {
    meter.create_histogram("http.client.request.duration", "Duration of HTTP client requests.", "s")
}

/// Metric: http.client.request.duration
/// Brief: Duration of HTTP client requests.
/// Unit: s
#[derive(Debug)]
pub struct HttpClientRequestDuration<T>(opentelemetry::metrics::Histogram<T>);

/// Attributes for the `http.client.request.duration` metric.
#[derive(Debug, Clone)]
pub struct HttpClientRequestDurationAttributes {
    /// HTTP request method.
    pub http_request_method: crate::attributes::http::HttpRequestMethod,
    /// [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6).
    pub http_response_status_code: Option<i64>,
    /// Describes a class of error the operation ended with.
    pub error_type: Option<crate::attributes::error::ErrorType>,
    /// Host identifier of the ["URI origin"](https://www.rfc-editor.org/rfc/rfc9110.html#name-uri-origin) HTTP request is sent to.
    pub server_address: String,
    /// Port identifier of the ["URI origin"](https://www.rfc-editor.org/rfc/rfc9110.html#name-uri-origin) HTTP request is sent to.
    pub server_port: i64,
    /// [OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.
    pub network_protocol_name: Option<String>,
    /// The actual version of the protocol used for network communication.
    pub network_protocol_version: Option<String>,
    /// The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.
    pub url_scheme: Option<String>,
    
}

impl <T> HttpClientRequestDuration<T> {
    pub fn new(meter: &opentelemetry::metrics::Meter) -> Self
        where opentelemetry::metrics::Meter: HistogramProvider<T>{
        Self(meter.create_histogram("http.client.request.duration", "Duration of HTTP client requests.", "s"))
    }

    /// Adds an additional value to the distribution.
    pub fn record(&self, value: T, attributes: HttpClientRequestDurationAttributes) {
        // self.0.record(value, attributes.into())
    }
}

/// Size of HTTP client request bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_http_client_request_body_size<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<T>
    where opentelemetry::metrics::Meter: HistogramProvider<T> {
    meter.create_histogram("http.client.request.body.size", "Size of HTTP client request bodies.", "By")
}

/// Metric: http.client.request.body.size
/// Brief: Size of HTTP client request bodies.
/// Unit: By
#[derive(Debug)]
pub struct HttpClientRequestBodySize<T>(opentelemetry::metrics::Histogram<T>);

/// Attributes for the `http.client.request.body.size` metric.
#[derive(Debug, Clone)]
pub struct HttpClientRequestBodySizeAttributes {
    /// HTTP request method.
    pub http_request_method: crate::attributes::http::HttpRequestMethod,
    /// [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6).
    pub http_response_status_code: Option<i64>,
    /// Describes a class of error the operation ended with.
    pub error_type: Option<crate::attributes::error::ErrorType>,
    /// Host identifier of the ["URI origin"](https://www.rfc-editor.org/rfc/rfc9110.html#name-uri-origin) HTTP request is sent to.
    pub server_address: String,
    /// Port identifier of the ["URI origin"](https://www.rfc-editor.org/rfc/rfc9110.html#name-uri-origin) HTTP request is sent to.
    pub server_port: i64,
    /// [OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.
    pub network_protocol_name: Option<String>,
    /// The actual version of the protocol used for network communication.
    pub network_protocol_version: Option<String>,
    /// The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.
    pub url_scheme: Option<String>,
    
}

impl <T> HttpClientRequestBodySize<T> {
    pub fn new(meter: &opentelemetry::metrics::Meter) -> Self
        where opentelemetry::metrics::Meter: HistogramProvider<T>{
        Self(meter.create_histogram("http.client.request.body.size", "Size of HTTP client request bodies.", "By"))
    }

    /// Adds an additional value to the distribution.
    pub fn record(&self, value: T, attributes: HttpClientRequestBodySizeAttributes) {
        // self.0.record(value, attributes.into())
    }
}

/// Size of HTTP client response bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_http_client_response_body_size<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<T>
    where opentelemetry::metrics::Meter: HistogramProvider<T> {
    meter.create_histogram("http.client.response.body.size", "Size of HTTP client response bodies.", "By")
}

/// Metric: http.client.response.body.size
/// Brief: Size of HTTP client response bodies.
/// Unit: By
#[derive(Debug)]
pub struct HttpClientResponseBodySize<T>(opentelemetry::metrics::Histogram<T>);

/// Attributes for the `http.client.response.body.size` metric.
#[derive(Debug, Clone)]
pub struct HttpClientResponseBodySizeAttributes {
    /// HTTP request method.
    pub http_request_method: crate::attributes::http::HttpRequestMethod,
    /// [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6).
    pub http_response_status_code: Option<i64>,
    /// Describes a class of error the operation ended with.
    pub error_type: Option<crate::attributes::error::ErrorType>,
    /// Host identifier of the ["URI origin"](https://www.rfc-editor.org/rfc/rfc9110.html#name-uri-origin) HTTP request is sent to.
    pub server_address: String,
    /// Port identifier of the ["URI origin"](https://www.rfc-editor.org/rfc/rfc9110.html#name-uri-origin) HTTP request is sent to.
    pub server_port: i64,
    /// [OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.
    pub network_protocol_name: Option<String>,
    /// The actual version of the protocol used for network communication.
    pub network_protocol_version: Option<String>,
    /// The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.
    pub url_scheme: Option<String>,
    
}

impl <T> HttpClientResponseBodySize<T> {
    pub fn new(meter: &opentelemetry::metrics::Meter) -> Self
        where opentelemetry::metrics::Meter: HistogramProvider<T>{
        Self(meter.create_histogram("http.client.response.body.size", "Size of HTTP client response bodies.", "By"))
    }

    /// Adds an additional value to the distribution.
    pub fn record(&self, value: T, attributes: HttpClientResponseBodySizeAttributes) {
        // self.0.record(value, attributes.into())
    }
}

/// Number of outbound HTTP connections that are currently active or idle on the client.
#[cfg(feature = "semconv_experimental")]
pub fn create_http_client_open_connections<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::UpDownCounter<T>
    where opentelemetry::metrics::Meter: UpDownCounterProvider<T> {
    meter.create_up_down_counter("http.client.open_connections", "Number of outbound HTTP connections that are currently active or idle on the client.", "{connection}")
}

/// The duration of the successfully established outbound HTTP connections.
#[cfg(feature = "semconv_experimental")]
pub fn create_http_client_connection_duration<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<T>
    where opentelemetry::metrics::Meter: HistogramProvider<T> {
    meter.create_histogram("http.client.connection.duration", "The duration of the successfully established outbound HTTP connections.", "s")
}

/// Metric: http.client.connection.duration
/// Brief: The duration of the successfully established outbound HTTP connections.
/// Unit: s
#[derive(Debug)]
pub struct HttpClientConnectionDuration<T>(opentelemetry::metrics::Histogram<T>);

/// Attributes for the `http.client.connection.duration` metric.
#[derive(Debug, Clone)]
pub struct HttpClientConnectionDurationAttributes {
    /// Port identifier of the ["URI origin"](https://www.rfc-editor.org/rfc/rfc9110.html#name-uri-origin) HTTP request is sent to.
    pub server_port: i64,
    /// Peer address of the network connection - IP address or Unix domain socket name.
    pub network_peer_address: Option<String>,
    /// The actual version of the protocol used for network communication.
    pub network_protocol_version: Option<String>,
    /// The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.
    pub url_scheme: Option<String>,
    /// Server domain name if available without reverse DNS lookup; otherwise, IP address or Unix domain socket name.
    pub server_address: String,
    
}

impl <T> HttpClientConnectionDuration<T> {
    pub fn new(meter: &opentelemetry::metrics::Meter) -> Self
        where opentelemetry::metrics::Meter: HistogramProvider<T>{
        Self(meter.create_histogram("http.client.connection.duration", "The duration of the successfully established outbound HTTP connections.", "s"))
    }

    /// Adds an additional value to the distribution.
    pub fn record(&self, value: T, attributes: HttpClientConnectionDurationAttributes) {
        // self.0.record(value, attributes.into())
    }
}

/// Number of active HTTP requests.
#[cfg(feature = "semconv_experimental")]
pub fn create_http_client_active_requests<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::UpDownCounter<T>
    where opentelemetry::metrics::Meter: UpDownCounterProvider<T> {
    meter.create_up_down_counter("http.client.active_requests", "Number of active HTTP requests.", "{request}")
}
