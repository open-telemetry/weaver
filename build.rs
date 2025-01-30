// SPDX-License-Identifier: Apache-2.0

//! A build script to generate the gRPC OTLP receiver API (client and server stubs.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The gRPC OTLP Receiver is vendored in `src/otlp_receiver/receiver` to avoid
    // depending on protoc in GitHub Actions.
    //
    // To regenerate the gRPC API from the proto file:
    // - Uncomment the following lines.
    // - Run `cargo build` to regenerate the API.
    // - Comment the following lines.
    // - Commit the changes.

    tonic_build::configure()
        .build_client(false)
        .out_dir("src/registry/otlp/grpc_stubs")
        .compile_protos(
            &[
                "src/registry/otlp/proto/opentelemetry/proto/collector/logs/v1/logs_service.proto",
                "src/registry/otlp/proto/opentelemetry/proto/collector/metrics/v1/metrics_service.proto",
                "src/registry/otlp/proto/opentelemetry/proto/collector/trace/v1/trace_service.proto",
            ],
            &["src/registry/otlp/proto"],
        )?;

    Ok(())
}
