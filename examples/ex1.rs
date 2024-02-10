// SPDX-License-Identifier: Apache-2.0

//! Example 1

// use crate::otel::meter::{http, HttpAttrs, HttpMetrics, JvmThreadCountAttrs};
// use crate::otel::tracer::{HttpRequestAttrs, HttpRequestEvent, HttpRequestOptAttrs, Status};

// mod otel;

fn main() {
    // // Starts a new span with the required attributes.
    // // todo 2 start impl: 1) with, 2) without required attributes
    // let mut span1 = otel::tracer::start_http_request(
    //     HttpRequestAttrs {
    //         url_host: "localhost".to_string(),
    //     });
    //
    // // Specifies some optional attributes.
    // span1.attr_url_scheme("https".to_string());
    // span1.attr_client_port(443);
    //
    // // Add an event to the span.
    // span1.event(HttpRequestEvent::Error {
    //     exception_type: None,
    //     exception_message: Some("an error message".into()),
    //     exception_stacktrace: None,
    // });
    //
    // // Set the status of the span.
    // span1.status(Status::Ok);
    // // End the span. After this call, the span is not longer
    // // accessible.
    // span1.end();
    //
    // // At this point, any reference to the span1 will result in a compiler
    // // error.
    //
    // // ========================================================================
    // // Starts a new span with the required attributes.
    // let mut span2 = otel::tracer::start_http_request(
    //     HttpRequestAttrs {
    //         url_host: "localhost".to_string(),
    //     });
    // span2.event(HttpRequestEvent::Error {
    //     exception_type: None,
    //     exception_message: None,
    //     exception_stacktrace: None,
    // });
    // span2.status(Status::Ok);
    // // End the span with optional attributes.
    // span2.end_with_opt_attrs(HttpRequestOptAttrs {
    //     url_scheme: Some("https".to_string()),
    //     client_port: Some(443),
    //     ..Default::default()
    // });
    //
    // // ========================================================================
    // // Reports an HTTP Request event.
    // otel::eventer::event_http_request(otel::eventer::HttpRequestAttrs {
    //     server_address: Some("localhost".to_string()),
    //     server_port: Some(443),
    //     network_protocol_name: Some("http".to_string()),
    //     network_protocol_version: None,
    //     url_scheme: None,
    //     url_host: "".to_string(),
    // });
    // // ========================================================================
    // // Reports an HTTP Response event.
    // otel::eventer::event_http_response(otel::eventer::HttpResponseAttrs {
    //     server_address: Some("localhost".to_string()),
    //     server_port: Some(443),
    //     http_response_status_code: Some(200),
    //     network_protocol_name: Some("http".to_string()),
    //     network_protocol_version: None,
    //     url_scheme: None,
    //     url_host: "".to_string(),
    // });
    //
    //
    // // ========================================================================
    // // Example of univariate metrics.
    // // todo otel::meter::new_jvm_thread_count
    // let mut jvm_thread_count = otel::meter::jvm_thread_count_u64();
    // jvm_thread_count.add(10, JvmThreadCountAttrs { thread_daemon: Some(true) });
    //
    // let mut http_server = otel::meter::http_server_request_duration_f64();
    // http_server.record(10.0, otel::meter::HttpServerRequestDurationAttrs {
    //     server_address: Some("localhost".to_string()),
    //     server_port: Some(443),
    //     http_response_status_code: Some(200),
    //     network_protocol_name: Some("http".to_string()),
    //     network_protocol_version: None,
    //     url_scheme: None,
    // });
    //
    // // ========================================================================
    // // Example of multivariate metrics.
    // // todo otel::meter::new_http
    // // todo check concept of metric_group
    // let mut http = otel::meter::http();
    // http.report(
    //     HttpMetrics {
    //         jvm_thread_count: 10,
    //         jvm_class_loaded: 50,
    //         jvm_cpu_recent_utilization: 60,
    //     },
    //     HttpAttrs {
    //         server_address: Some("localhost".into()),
    //         server_port: Some(8080),
    //         http_response_status_code: None,
    //         network_protocol_name: None,
    //         network_protocol_version: None,
    //         url_scheme: None,
    //         url_host: "".to_string(),
    //     },
    // );
}
