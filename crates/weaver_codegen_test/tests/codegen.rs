// SPDX-License-Identifier: Apache-2.0

//! Test code generation

use opentelemetry::global;
use opentelemetry::metrics::Histogram;
use crate::attributes::client;
use crate::metrics::http::create_http_client_request_duration;
use crate::attributes::http::HTTP_REQUEST_METHOD;
use crate::metrics::http::HttpServerRequestDuration;
use crate::metrics::http::HttpServerRequestDurationReqAttributes;
use crate::metrics::http::HttpServerRequestDurationOptAttributes;
use crate::metrics::http::HttpClientActiveRequests;
use crate::metrics::http::HttpClientActiveRequestsReqAttributes;
use crate::metrics::system::SystemCpuTime;
use crate::metrics::system::SystemCpuTimeOptAttributes;
use crate::attributes::system::SystemCpuState;
use crate::metrics::system::SystemCpuUtilization;
use crate::metrics::system::SystemCpuUtilizationOptAttributes;

pub mod attributes {
    include!(concat!(env!("OUT_DIR"), "/attributes/mod.rs"));
    pub mod client {
        include!(concat!(env!("OUT_DIR"), "/attributes/client.rs"));
    }
    pub mod error {
        include!(concat!(env!("OUT_DIR"), "/attributes/error.rs"));
    }
    pub mod exception {
        include!(concat!(env!("OUT_DIR"), "/attributes/exception.rs"));
    }
    pub mod http {
        include!(concat!(env!("OUT_DIR"), "/attributes/http.rs"));
    }
    pub mod network {
        include!(concat!(env!("OUT_DIR"), "/attributes/network.rs"));
    }
    pub mod server {
        include!(concat!(env!("OUT_DIR"), "/attributes/server.rs"));
    }
    pub mod system {
        include!(concat!(env!("OUT_DIR"), "/attributes/system.rs"));
    }
    pub mod url {
        include!(concat!(env!("OUT_DIR"), "/attributes/url.rs"));
    }
}

pub mod metrics {
    include!(concat!(env!("OUT_DIR"), "/metrics/mod.rs"));
    pub mod http {
        include!(concat!(env!("OUT_DIR"), "/metrics/http.rs"));
    }
    pub mod system {
        include!(concat!(env!("OUT_DIR"), "/metrics/system.rs"));
    }
}

    use crate::attributes::http::HttpRequestMethod;

#[test]
fn test_codegen() {
    println!("{:?}", client::CLIENT_ADDRESS.value("145.34.23.56".into()));
    println!("{:?}", client::CLIENT_ADDRESS.key());
    println!("{:?}", client::CLIENT_PORT.value(8080));
    println!("{:?}", client::CLIENT_PORT.key());

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

    // All the tests passed, remove the generated files
    //remove_generated_files();
}