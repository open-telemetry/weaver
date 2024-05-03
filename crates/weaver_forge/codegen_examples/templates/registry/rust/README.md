# Semantic Conventions for Rust

# Usage

```rust
use opentelemetry::KeyValue;
use opentelemetry::metrics::{Histogram, MeterProvider};
use opentelemetry_sdk::{Resource, runtime};
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};

use semconv::attributes;
use semconv::attributes::http::{HTTP_REQUEST_METHOD, HttpRequestMethod};
use semconv::attributes::system::SystemCpuState;
use semconv::metrics::http::{create_http_client_request_duration, HttpClientActiveRequests, HttpClientActiveRequestsReqAttributes, HttpServerRequestDurationOptAttributes, HttpServerRequestDurationReqAttributes};
use semconv::metrics::http::HttpServerRequestDuration;
use semconv::metrics::system::{SystemCpuTime, SystemCpuTimeOptAttributes, SystemCpuUtilization, SystemCpuUtilizationOptAttributes};

/// Main
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let meter_provider = init_meter_provider();

    // SemConv attributes are typed, so the compiler will catch type errors
    // Experimental attributes are not visible if the `semconv_experimental` feature is not enabled
    println!("{:?}", attributes::client::CLIENT_ADDRESS.value("145.34.23.56".into()));
    println!("{:?}", attributes::client::CLIENT_ADDRESS.key());
    println!("{:?}", attributes::client::CLIENT_PORT.value(8080));
    println!("{:?}", attributes::client::CLIENT_PORT.key());

    println!("{}", HttpRequestMethod::Connect);

    let meter = meter_provider.meter("mylibname");

    // Create a u64 http.client.request.duration metric
    let http_client_request_duration: Histogram<u64> = create_http_client_request_duration(&meter);
    http_client_request_duration.record(100, &[
        HTTP_REQUEST_METHOD.value(&HttpRequestMethod::Connect),
        // here nothing guarantees that all the required attributes are provided
    ]);

    let http_client_request_duration: Histogram<f64> = create_http_client_request_duration(&meter);
    dbg!(http_client_request_duration);

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

    // Create a f64 http.server.request.duration metric (as defined in the OpenTelemetry HTTP
    // semantic conventions)
    let http_client_active_requests = HttpClientActiveRequests::<f64>::new(&meter);

    // Adds a new data point and provide the required attributes. Optional attributes are not
    // provided in this example.
    http_client_active_requests.add(10.0, &HttpClientActiveRequestsReqAttributes {
        server_address: "10.0.0.1".to_owned(),
        server_port: 8080,
    }, None);

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

    meter_provider.shutdown()?;
    Ok(())
}

fn init_meter_provider() -> SdkMeterProvider {
    let exporter = opentelemetry_stdout::MetricsExporterBuilder::default()
        .with_encoder(|writer, data|
            Ok(serde_json::to_writer_pretty(writer, &data).unwrap()))
        .build();
    let reader = PeriodicReader::builder(exporter, runtime::Tokio).build();
    SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(Resource::new(vec![KeyValue::new(
            "service.name",
            "metrics-basic-example",
        )]))
        .build()
}
```