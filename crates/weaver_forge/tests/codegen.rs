// SPDX-License-Identifier: Apache-2.0

//! Tests for the codegen module

use opentelemetry::global;
use opentelemetry::metrics::Histogram;

use attributes::http::{HTTP_REQUEST_METHOD, HttpRequestMethod};
use attributes::system::SystemCpuState;
use metrics::http::{create_http_client_request_duration, HttpClientActiveRequests, HttpClientActiveRequestsReqAttributes, HttpServerRequestDurationOptAttributes, HttpServerRequestDurationReqAttributes};
use metrics::http::HttpServerRequestDuration;
use metrics::system::{SystemCpuTime, SystemCpuTimeOptAttributes, SystemCpuUtilization, SystemCpuUtilizationOptAttributes};

pub mod attributes;
pub mod metrics;

#[test]
fn test_semconv_rust_codegen() {
    // SemConv attributes are typed, so the compiler will catch type errors
    // Experimental attributes are not visible if the `semconv_experimental` feature is not enabled
    println!("{:?}", attributes::client::CLIENT_ADDRESS.value("145.34.23.56".into()));
    println!("{:?}", attributes::client::CLIENT_ADDRESS.key());
    println!("{:?}", attributes::client::CLIENT_PORT.value(8080));
    println!("{:?}", attributes::client::CLIENT_PORT.key());

    println!("{}", HttpRequestMethod::Connect);

    let meter = global::meter("mylibname");

    // Create a u64 http.client.request.duration metric
    let http_client_request_duration: Histogram<u64> = create_http_client_request_duration(&meter);
    http_client_request_duration.record(100, &[
        HTTP_REQUEST_METHOD.value(&HttpRequestMethod::Connect),
        // here nothing guarantees that all the required attributes are provided
    ]);

    let http_client_request_duration: Histogram<f64> = create_http_client_request_duration(&meter);
    dbg!(http_client_request_duration);

    // ==== A TYPE-SAFE HISTOGRAM API ====
    // Create a u64 http.server.request.duration metric (as defined in the OpenTelemetry HTTP
    // semantic conventions)
    let http_request_duration = HttpServerRequestDuration::<u64>::new(&meter);

    // Records a new data point and provide the required and some optional attributes
    http_request_duration.record(100, &HttpServerRequestDurationReqAttributes {
        http_request_method: HttpRequestMethod::Connect,
        url_scheme: "http".to_owned(),
    }, Some(&HttpServerRequestDurationOptAttributes {
        http_response_status_code: Some(200),
        ..Default::default()
    }));

    // ==== A TYPE-SAFE UP-DOWN-COUNTER API ====
    // Create a f64 http.server.request.duration metric (as defined in the OpenTelemetry HTTP
    // semantic conventions)
    let http_client_active_requests = HttpClientActiveRequests::<f64>::new(&meter);

    // Adds a new data point and provide the required attributes. Optional attributes are not
    // provided in this example.
    http_client_active_requests.add(10.0, &HttpClientActiveRequestsReqAttributes {
        server_address: "10.0.0.1".to_owned(),
        server_port: 8080,
    }, None);

    // ==== A TYPE-SAFE COUNTER API ====
    // Create a f64 system.cpu.time metric (as defined in the OpenTelemetry System semantic
    // conventions)
    let system_cpu_time = SystemCpuTime::<f64>::new(&meter);

    // Adds a new data point and provide some optional attributes.
    // Note: In the method signature, there is no required attribute.
    system_cpu_time.add(10.0, Some(&SystemCpuTimeOptAttributes {
        system_cpu_logical_number: Some(0),
        system_cpu_state: Some(SystemCpuState::Idle)
    }));
    // Adds a new data point with a custom CPU state.
    system_cpu_time.add(20.0, Some(&SystemCpuTimeOptAttributes {
        system_cpu_logical_number: Some(0),
        system_cpu_state: Some(SystemCpuState::_Custom("custom".to_owned()))
    }));

    // ==== A TYPE-SAFE GAUGE API ====
    // Create a i64 system.cpu.utilization metric (as defined in the OpenTelemetry System semantic
    // conventions)
    let system_cpu_utilization = SystemCpuUtilization::<i64>::new(&meter);

    // Adds a new data point with no optional attributes.
    system_cpu_utilization.record(-5, None);
    // Adds a new data point with some optional attributes.
    system_cpu_utilization.record(10, Some(&SystemCpuUtilizationOptAttributes {
        system_cpu_logical_number: Some(0),
        system_cpu_state: Some(SystemCpuState::Idle)
    }));
}