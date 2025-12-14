// SPDX-License-Identifier: Apache-2.0

//! A build script to generate the gRPC OTLP receiver API (client and server stubs.

use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The gRPC OTLP Receiver is vendored in `src/otlp_receiver/receiver` to avoid
    // depending on protoc in GitHub Actions.
    //
    // To regenerate the gRPC API from the proto file:
    // - Uncomment the following lines.
    // - Run `cargo build` to regenerate the API.
    // - Comment the following lines.
    // - Commit the changes.

    // tonic_prost_build::configure()
    //     .out_dir("src/registry/otlp/grpc_stubs")
    //     .compile_protos(
    //         &[
    //             "src/registry/otlp/proto/opentelemetry/proto/collector/logs/v1/logs_service.proto",
    //             "src/registry/otlp/proto/opentelemetry/proto/collector/metrics/v1/metrics_service.proto",
    //             "src/registry/otlp/proto/opentelemetry/proto/collector/trace/v1/trace_service.proto",
    //         ],
    //         &["src/registry/otlp/proto"],
    //     )?;

    // Build the UI
    build_ui()?;

    Ok(())
}

fn build_ui() -> Result<(), Box<dyn std::error::Error>> {
    let ui_dir = std::path::Path::new("ui");

    // Get the npm command - on Windows it's npm.cmd, on Unix it's npm
    let npm_cmd = if cfg!(target_os = "windows") {
        "npm.cmd"
    } else {
        "npm"
    };

    // Check if npm is available
    let npm_check = Command::new(npm_cmd)
        .arg("--version")
        .output();

    if npm_check.is_err() {
        return Err(
            "npm not found. Please install Node.js and npm from https://nodejs.org/ to build this project."
                .into(),
        );
    }

    println!("cargo:warning=Building UI...");

    // Install dependencies if node_modules doesn't exist
    let node_modules = ui_dir.join("node_modules");
    if !node_modules.exists() {
        println!("cargo:warning=Installing UI dependencies...");
        let status = Command::new(npm_cmd)
            .arg("install")
            .current_dir(ui_dir)
            .status()?;

        if !status.success() {
            return Err("Failed to install UI dependencies".into());
        }
    }

    // Build the UI
    let status = Command::new(npm_cmd)
        .arg("run")
        .arg("build")
        .current_dir(ui_dir)
        .status()?;

    if !status.success() {
        return Err("Failed to build UI".into());
    }

    println!("cargo:warning=UI build complete");

    Ok(())
}
