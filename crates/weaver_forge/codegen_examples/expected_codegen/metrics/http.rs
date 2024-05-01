/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/metrics/metrics.rs.j2



/// Duration of HTTP server requests.
pub fn create_u64_http_server_request_duration(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<u64> {
    meter.u64_histogram("http.server.request.duration")
        .with_description("Duration of HTTP server requests.")
        .with_unit(opentelemetry::metrics::Unit::new("s"))
        .init()
}


/// Duration of HTTP server requests.
pub fn create_f64_http_server_request_duration(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<f64> {
    meter.f64_histogram("http.server.request.duration")
        .with_description("Duration of HTTP server requests.")
        .with_unit(opentelemetry::metrics::Unit::new("s"))
        .init()
}



/// Number of active HTTP server requests.
#[cfg(feature = "semconv_experimental")]
pub fn create_i64_http_server_active_requests(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::UpDownCounter<i64> {
    meter.i64_up_down_counter("http.server.active_requests")
        .with_description("Number of active HTTP server requests.")
        .with_unit(opentelemetry::metrics::Unit::new("{request}"))
        .init()
}


/// Number of active HTTP server requests.
#[cfg(feature = "semconv_experimental")]
pub fn create_f64_http_server_active_requests(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::UpDownCounter<f64> {
    meter.f64_up_down_counter("http.server.active_requests")
        .with_description("Number of active HTTP server requests.")
        .with_unit(opentelemetry::metrics::Unit::new("{request}"))
        .init()
}



/// Size of HTTP server request bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_u64_http_server_request_body_size(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<u64> {
    meter.u64_histogram("http.server.request.body.size")
        .with_description("Size of HTTP server request bodies.")
        .with_unit(opentelemetry::metrics::Unit::new("By"))
        .init()
}


/// Size of HTTP server request bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_f64_http_server_request_body_size(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<f64> {
    meter.f64_histogram("http.server.request.body.size")
        .with_description("Size of HTTP server request bodies.")
        .with_unit(opentelemetry::metrics::Unit::new("By"))
        .init()
}



/// Size of HTTP server response bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_u64_http_server_response_body_size(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<u64> {
    meter.u64_histogram("http.server.response.body.size")
        .with_description("Size of HTTP server response bodies.")
        .with_unit(opentelemetry::metrics::Unit::new("By"))
        .init()
}


/// Size of HTTP server response bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_f64_http_server_response_body_size(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<f64> {
    meter.f64_histogram("http.server.response.body.size")
        .with_description("Size of HTTP server response bodies.")
        .with_unit(opentelemetry::metrics::Unit::new("By"))
        .init()
}



/// Duration of HTTP client requests.
pub fn create_u64_http_client_request_duration(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<u64> {
    meter.u64_histogram("http.client.request.duration")
        .with_description("Duration of HTTP client requests.")
        .with_unit(opentelemetry::metrics::Unit::new("s"))
        .init()
}


/// Duration of HTTP client requests.
pub fn create_f64_http_client_request_duration(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<f64> {
    meter.f64_histogram("http.client.request.duration")
        .with_description("Duration of HTTP client requests.")
        .with_unit(opentelemetry::metrics::Unit::new("s"))
        .init()
}



/// Size of HTTP client request bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_u64_http_client_request_body_size(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<u64> {
    meter.u64_histogram("http.client.request.body.size")
        .with_description("Size of HTTP client request bodies.")
        .with_unit(opentelemetry::metrics::Unit::new("By"))
        .init()
}


/// Size of HTTP client request bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_f64_http_client_request_body_size(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<f64> {
    meter.f64_histogram("http.client.request.body.size")
        .with_description("Size of HTTP client request bodies.")
        .with_unit(opentelemetry::metrics::Unit::new("By"))
        .init()
}



/// Size of HTTP client response bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_u64_http_client_response_body_size(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<u64> {
    meter.u64_histogram("http.client.response.body.size")
        .with_description("Size of HTTP client response bodies.")
        .with_unit(opentelemetry::metrics::Unit::new("By"))
        .init()
}


/// Size of HTTP client response bodies.
#[cfg(feature = "semconv_experimental")]
pub fn create_f64_http_client_response_body_size(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<f64> {
    meter.f64_histogram("http.client.response.body.size")
        .with_description("Size of HTTP client response bodies.")
        .with_unit(opentelemetry::metrics::Unit::new("By"))
        .init()
}



/// Number of outbound HTTP connections that are currently active or idle on the client.
#[cfg(feature = "semconv_experimental")]
pub fn create_i64_http_client_open_connections(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::UpDownCounter<i64> {
    meter.i64_up_down_counter("http.client.open_connections")
        .with_description("Number of outbound HTTP connections that are currently active or idle on the client.")
        .with_unit(opentelemetry::metrics::Unit::new("{connection}"))
        .init()
}


/// Number of outbound HTTP connections that are currently active or idle on the client.
#[cfg(feature = "semconv_experimental")]
pub fn create_f64_http_client_open_connections(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::UpDownCounter<f64> {
    meter.f64_up_down_counter("http.client.open_connections")
        .with_description("Number of outbound HTTP connections that are currently active or idle on the client.")
        .with_unit(opentelemetry::metrics::Unit::new("{connection}"))
        .init()
}



/// The duration of the successfully established outbound HTTP connections.
#[cfg(feature = "semconv_experimental")]
pub fn create_u64_http_client_connection_duration(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<u64> {
    meter.u64_histogram("http.client.connection.duration")
        .with_description("The duration of the successfully established outbound HTTP connections.")
        .with_unit(opentelemetry::metrics::Unit::new("s"))
        .init()
}


/// The duration of the successfully established outbound HTTP connections.
#[cfg(feature = "semconv_experimental")]
pub fn create_f64_http_client_connection_duration(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Histogram<f64> {
    meter.f64_histogram("http.client.connection.duration")
        .with_description("The duration of the successfully established outbound HTTP connections.")
        .with_unit(opentelemetry::metrics::Unit::new("s"))
        .init()
}



/// Number of active HTTP requests.
#[cfg(feature = "semconv_experimental")]
pub fn create_i64_http_client_active_requests(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::UpDownCounter<i64> {
    meter.i64_up_down_counter("http.client.active_requests")
        .with_description("Number of active HTTP requests.")
        .with_unit(opentelemetry::metrics::Unit::new("{request}"))
        .init()
}


/// Number of active HTTP requests.
#[cfg(feature = "semconv_experimental")]
pub fn create_f64_http_client_active_requests(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::UpDownCounter<f64> {
    meter.f64_up_down_counter("http.client.active_requests")
        .with_description("Number of active HTTP requests.")
        .with_unit(opentelemetry::metrics::Unit::new("{request}"))
        .init()
}

