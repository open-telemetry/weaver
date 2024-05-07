// SPDX-License-Identifier: Apache-2.0

//! This integration test aims to validate the code generation from a semantic convention registry.
//! By using an integration test, we confirm that the interface of the generated code is public. We
//! also verify that the entirety of the generated code is compilable and exposes the expected
//! constants, structs, enums, and functions.

// Include the generated code
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

use crate::attributes::client;
use crate::attributes::http::HttpRequestMethod;
use crate::attributes::http::HTTP_REQUEST_METHOD;
use crate::attributes::system::SystemCpuState;
use crate::metrics::http::create_http_client_request_duration;
use crate::metrics::http::HttpClientActiveRequests;
use crate::metrics::http::HttpClientActiveRequestsReqAttributes;
use crate::metrics::http::HttpServerRequestDuration;
use crate::metrics::http::HttpServerRequestDurationOptAttributes;
use crate::metrics::http::HttpServerRequestDurationReqAttributes;
use crate::metrics::system::SystemCpuTime;
use crate::metrics::system::SystemCpuTimeOptAttributes;
use crate::metrics::system::SystemCpuUtilization;
use crate::metrics::system::SystemCpuUtilizationOptAttributes;
use opentelemetry::metrics::Histogram;
use opentelemetry::{global, KeyValue};

#[test]
fn test_codegen() {
    // Test the constants generated for the attributes
    // In the generated API the attributes are typed, so the compiler will catch type errors
    assert_eq!(client::CLIENT_ADDRESS.key().as_str(), "client.address");
    assert_eq!(
        client::CLIENT_ADDRESS.value("145.34.23.56".into()),
        KeyValue::new("client.address", "145.34.23.56")
    );
    assert_eq!(client::CLIENT_PORT.key().as_str(), "client.port");
    assert_eq!(
        client::CLIENT_PORT.value(8080),
        KeyValue::new("client.port", 8080)
    );

    // Enum values are also generated
    assert_eq!(HttpRequestMethod::Connect.as_str(), "CONNECT");
    assert_eq!(HttpRequestMethod::Delete.as_str(), "DELETE");
    assert_eq!(
        HttpRequestMethod::_Custom("UNKNOWN_METHOD".to_owned()).as_str(),
        "UNKNOWN_METHOD"
    );

    // Create an OpenTelemetry meter
    let meter = global::meter("my_meter");

    // Create a u64 http.client.request.duration metric and record a data point.
    // This is the low-level API, where:
    // - the required attributes are not enforced by the compiler.
    // - the attributes provided are not checked for correctness by the compiler (i.e. the
    // attributes specified in the original semantic convention
    let http_client_request_duration: Histogram<u64> = create_http_client_request_duration(&meter);
    http_client_request_duration.record(
        100,
        &[HTTP_REQUEST_METHOD.value(&HttpRequestMethod::Connect)],
    );

    // Create a f64 http.client.request.duration metric and record a data point.
    let http_client_request_duration: Histogram<f64> = create_http_client_request_duration(&meter);
    http_client_request_duration.record(
        100.0,
        &[HTTP_REQUEST_METHOD.value(&HttpRequestMethod::Connect)],
    );

    // ==== A TYPE-SAFE HISTOGRAM API ====
    // Create a u64 http.server.request.duration metric (as defined in the OpenTelemetry HTTP
    // semantic conventions).
    // The API is type-safe, so the compiler will catch type errors. The required attributes are
    // enforced by the compiler. All the attributes provided are checked for correctness by the
    // compiler in relation to the original semantic convention.
    let http_request_duration = HttpServerRequestDuration::<u64>::new(&meter);
    // Records a new data point and provide the required and some optional attributes
    http_request_duration.record(
        100,
        &HttpServerRequestDurationReqAttributes {
            http_request_method: HttpRequestMethod::Connect,
            url_scheme: "http".to_owned(),
        },
        Some(&HttpServerRequestDurationOptAttributes {
            http_response_status_code: Some(200),
            ..Default::default()
        }),
    );

    // ==== A TYPE-SAFE UP-DOWN-COUNTER API ====
    // Create a f64 http.server.request.duration metric (as defined in the OpenTelemetry HTTP
    // semantic conventions)
    let http_client_active_requests = HttpClientActiveRequests::<f64>::new(&meter);
    // Adds a new data point and provide the required attributes. Optional attributes are not
    // provided in this example.
    http_client_active_requests.add(
        10.0,
        &HttpClientActiveRequestsReqAttributes {
            server_address: "10.0.0.1".to_owned(),
            server_port: 8080,
        },
        None,
    );

    // ==== A TYPE-SAFE COUNTER API ====
    // Create a f64 system.cpu.time metric (as defined in the OpenTelemetry System semantic
    // conventions)
    let system_cpu_time = SystemCpuTime::<f64>::new(&meter);
    // Adds a new data point and provide some optional attributes.
    // Note: In the method signature, there is no required attribute.
    system_cpu_time.add(
        10.0,
        Some(&SystemCpuTimeOptAttributes {
            system_cpu_logical_number: Some(0),
            system_cpu_state: Some(SystemCpuState::Idle),
        }),
    );
    // Adds a new data point with a custom CPU state.
    system_cpu_time.add(
        20.0,
        Some(&SystemCpuTimeOptAttributes {
            system_cpu_logical_number: Some(0),
            system_cpu_state: Some(SystemCpuState::_Custom("custom".to_owned())),
        }),
    );

    // ==== A TYPE-SAFE GAUGE API ====
    // Create a i64 system.cpu.utilization metric (as defined in the OpenTelemetry System semantic
    // conventions)
    let system_cpu_utilization = SystemCpuUtilization::<i64>::new(&meter);
    // Adds a new data point with no optional attributes.
    system_cpu_utilization.record(-5, None);
    // Adds a new data point with some optional attributes.
    system_cpu_utilization.record(
        10,
        Some(&SystemCpuUtilizationOptAttributes {
            system_cpu_logical_number: Some(0),
            system_cpu_state: Some(SystemCpuState::Idle),
        }),
    );
}
