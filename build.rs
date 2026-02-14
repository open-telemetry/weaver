// SPDX-License-Identifier: Apache-2.0

//! A build script to generate the gRPC OTLP receiver API (client and server stubs.

use std::{path::Path, time::SystemTime};

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
    check_ui()?;

    Ok(())
}

fn timestamp(dir: walkdir::DirEntry) -> Result<SystemTime, Box<dyn std::error::Error>> {
    let md = dir.metadata()?;
    Ok(md.modified()?)
}

/// Helper function to determine if package-lock file is out of date.
fn is_package_lock_stale(dir: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let lock_timestamp = dir.join("pnpm-lock.yaml").metadata()?.modified()?;
    let package_timestamp = dir.join("package.json").metadata()?.modified()?;
    if package_timestamp > lock_timestamp {
        return Ok(true);
    }
    Ok(false)
}

/// Helper function to determine if NPM project is out of date.
fn is_ui_stale(dir: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    // If any output directories don't exist, rebuild.
    if !dir.join("dist").exists() {
        return Ok(true);
    }
    if !dir.join("node_modules").exists() {
        return Ok(true);
    }
    // Now check source files. This may be a bit expensive, as we continuously check last modified.
    let last_build = dir.join("dist/index.html").metadata()?.modified()?;
    let lock_timestamp = dir.join("pnpm-lock.yaml").metadata()?.modified()?;
    if lock_timestamp > last_build {
        return Ok(true);
    }
    for entry in walkdir::WalkDir::new(dir.join("src")) {
        if timestamp(entry?)? > last_build {
            return Ok(true);
        }
    }
    Ok(false)
}

fn check_ui() -> Result<(), Box<dyn std::error::Error>> {
    let ui_dir = Path::new("ui");

    // Check if UI is out of date before running.
    if is_ui_stale(ui_dir)? {
        return Err(
            "Weaver UI is out of date. Please run `pnpm build` in the `ui` directory.".into(),
        );
    }

    // Check if we need to install packages.
    if is_package_lock_stale(ui_dir)? {
        // TODO - Disable for CI
        return Err("Weaver `ui/pnpm-lock.yaml` is out of date. Please run `pnpm install` in the `ui` directory.".into());
    }

    Ok(())
}
