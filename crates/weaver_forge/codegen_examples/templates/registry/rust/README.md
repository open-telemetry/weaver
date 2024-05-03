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

The execution of this program will generate the following output:

```
KeyValue { key: Static("client.address"), value: String(Static("145.34.23.56")) }
Static("client.address")
KeyValue { key: Static("client.port"), value: I64(8080) }
Static("client.port")
CONNECT
[src/main.rs:39:5] http_client_request_duration = Histogram<f64>
{
  "resourceMetrics": {
    "resource": {
      "attributes": [
        {
          "key": "service.name",
          "value": {
            "stringValue": "metrics-basic-example"
          }
        }
      ]
    },
    "scopeMetrics": [
      {
        "scope": {
          "name": "mylibname"
        },
        "metrics": [
          {
            "name": "http.client.request.duration",
            "description": "Duration of HTTP client requests.",
            "unit": "s",
            "histogram": {
              "dataPoints": [
                {
                  "attributes": {
                    "http.request.method": {
                      "stringValue": "CONNECT"
                    }
                  },
                  "startTimeUnixNano": 1714780164856054000,
                  "timeUnixNano": 1714780164856202000,
                  "startTime": "2024-05-03 23:49:24.856",
                  "time": "2024-05-03 23:49:24.856",
                  "count": 1,
                  "explicitBounds": [
                    0.0,
                    5.0,
                    10.0,
                    25.0,
                    50.0,
                    75.0,
                    100.0,
                    250.0,
                    500.0,
                    750.0,
                    1000.0,
                    2500.0,
                    5000.0,
                    7500.0,
                    10000.0
                  ],
                  "bucketCounts": [
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0
                  ],
                  "min": 100,
                  "max": 100,
                  "sum": 100,
                  "exemplars": [],
                  "flags": 0
                }
              ],
              "aggregationTemporality": "Cumulative"
            }
          },
          {
            "name": "http.server.request.duration",
            "description": "Duration of HTTP server requests.",
            "unit": "s",
            "histogram": {
              "dataPoints": [
                {
                  "attributes": {
                    "http.request.method": {
                      "stringValue": "CONNECT"
                    },
                    "http.response.status_code": {
                      "intValue": 200
                    },
                    "url.scheme": {
                      "stringValue": "http"
                    }
                  },
                  "startTimeUnixNano": 1714780164856111000,
                  "timeUnixNano": 1714780164856204000,
                  "startTime": "2024-05-03 23:49:24.856",
                  "time": "2024-05-03 23:49:24.856",
                  "count": 1,
                  "explicitBounds": [
                    0.0,
                    5.0,
                    10.0,
                    25.0,
                    50.0,
                    75.0,
                    100.0,
                    250.0,
                    500.0,
                    750.0,
                    1000.0,
                    2500.0,
                    5000.0,
                    7500.0,
                    10000.0
                  ],
                  "bucketCounts": [
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0
                  ],
                  "min": 100,
                  "max": 100,
                  "sum": 100,
                  "exemplars": [],
                  "flags": 0
                }
              ],
              "aggregationTemporality": "Cumulative"
            }
          },
          {
            "name": "http.client.active_requests",
            "description": "Number of active HTTP requests.",
            "unit": "{request}",
            "sum": {
              "dataPoints": [
                {
                  "attributes": {
                    "server.address": {
                      "stringValue": "10.0.0.1"
                    },
                    "server.port": {
                      "intValue": 8080
                    }
                  },
                  "startTime": "2024-05-03 23:49:24.856",
                  "time": "2024-05-03 23:49:24.856",
                  "startTimeUnixNano": 1714780164856139000,
                  "timeUnixNano": 1714780164856219000,
                  "value": 10.0
                }
              ],
              "aggregationTemporality": "Cumulative",
              "isMonotonic": false
            }
          },
          {
            "name": "system.cpu.time",
            "description": "Seconds each logical CPU spent on each mode",
            "unit": "s",
            "sum": {
              "dataPoints": [
                {
                  "attributes": {
                    "system.cpu.logical_number": {
                      "intValue": 0
                    },
                    "system.cpu.state": {
                      "stringValue": "idle"
                    }
                  },
                  "startTime": "2024-05-03 23:49:24.856",
                  "time": "2024-05-03 23:49:24.856",
                  "startTimeUnixNano": 1714780164856152000,
                  "timeUnixNano": 1714780164856220000,
                  "value": 10.0
                },
                {
                  "attributes": {
                    "system.cpu.logical_number": {
                      "intValue": 0
                    },
                    "system.cpu.state": {
                      "stringValue": "custom"
                    }
                  },
                  "startTime": "2024-05-03 23:49:24.856",
                  "time": "2024-05-03 23:49:24.856",
                  "startTimeUnixNano": 1714780164856152000,
                  "timeUnixNano": 1714780164856220000,
                  "value": 20.0
                }
              ],
              "aggregationTemporality": "Cumulative",
              "isMonotonic": true
            }
          },
          {
            "name": "system.cpu.utilization",
            "description": "Difference in system.cpu.time since the last measurement, divided by the elapsed time and number of logical CPUs",
            "unit": "1",
            "gauge": {
              "dataPoints": [
                {
                  "attributes": {
                    "system.cpu.logical_number": {
                      "intValue": 0
                    },
                    "system.cpu.state": {
                      "stringValue": "idle"
                    }
                  },
                  "startTime": null,
                  "time": "2024-05-03 23:49:24.856",
                  "startTimeUnixNano": null,
                  "timeUnixNano": 1714780164856176000,
                  "value": 10
                },
                {
                  "attributes": {},
                  "startTime": null,
                  "time": "2024-05-03 23:49:24.856",
                  "startTimeUnixNano": null,
                  "timeUnixNano": 1714780164856171000,
                  "value": -5
                }
              ]
            }
          }
        ]
      }
    ]
  }
}
```