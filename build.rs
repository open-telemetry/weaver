// SPDX-License-Identifier: Apache-2.0

//! A build script to generate the gRPC OTLP receiver API (client and server stubs.

use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, ExitStatus}, time::SystemTime,
};

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

/// Helper function to determine if NPM project is out of date.
fn is_ui_stale(dir: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    // If any output directories don't exist, rebuild.
    if !dir.join("dist").exists() {
        return Ok(true)
    }
    if !dir.join("node_modules").exists() {
        return Ok(true)
    }
    // If our lock file is out of date with our package file, we need to rebuild.
    let lock_timestamp = dir.join("package-lock.json").metadata()?.modified()?;
    let package_timestamp = dir.join("package.json").metadata()?.modified()?;
    if package_timestamp > lock_timestamp {
        return Ok(true)
    }
    // Now check source files. This may be a bit expensive, as we continuously check last modified.
    let last_build = dir.join("dist/index.html").metadata()?.modified()?;
    let mut last_source_time: Option<SystemTime> = None;
    for entry in walkdir::WalkDir::new(dir.join("src")) {
        if let Ok(entry) = entry {
            if let Ok(metadata) = entry.metadata() {
                // Get the modification time for the current file/directory
                if let Ok(modified_time) = metadata.modified() {
                    // Update latest_time if the current file is newer
                    if last_source_time.is_none() || modified_time > last_source_time.unwrap() {
                        last_source_time = Some(modified_time);
                    }
                }
            }
        }
    }
    if let Some(time) = last_source_time {
        return Ok(time > last_build);
    }
    Ok(false)
}

fn build_ui() -> Result<(), Box<dyn std::error::Error>> {
    let ui_dir = Path::new("ui");

    // Check if UI is out of date before running.
    if !is_ui_stale(ui_dir)? {
        return Ok(())
    }

    // Get the npm command - on Windows it's npm.cmd, on Unix it's npm
    let mut npm_runner = if cfg!(target_os = "windows") {
        NpmRunner::NpmExec("npm.cmd".to_owned())
    } else {
        NpmRunner::NpmExec("npm".to_owned())
    };

    if !npm_runner.check_valid() {
        println!("cargo:warning=npm not found. Please install Node.js and npm from https://nodejs.org/ to build this project.");
        println!("cargo:warning=Attempting to use docker for now.");
        // TODO - Docker usage should ALWAYS be behind some kind of flag, ideally.
        npm_runner = NpmRunner::Docker;
    }

    // Check if npm is available
    println!("cargo:warning=Building UI...");

    // Always update dependencies to exactly match latest package-lock.json
    println!("cargo:warning=Checking UI dependencies...");
    let status = npm_runner.run(ui_dir, vec!["ci"])?;

    if !status.success() {
        println!("cargo:warning=Unable to use installed npm, using docker instead...");
        npm_runner = NpmRunner::Docker;
        let status = npm_runner.run(ui_dir, vec!["ci"])?;
        if !status.success() {
            return Err("Failed to load UI dependencies".into());
        }
    }

    // Build the UI
    let status = npm_runner.run(ui_dir, vec!["run", "build"])?;

    if !status.success() {
        return Err("Failed to build UI".into());
    }

    println!("cargo:warning=UI build complete");

    Ok(())
}

enum NpmRunner {
    NpmExec(String),
    Docker,
}

impl NpmRunner {
    // Retruns true if this is a valid way to run NPM.
    fn check_valid(&self) -> bool {
        match self {
            NpmRunner::NpmExec(npm_cmd) => Command::new(npm_cmd).arg("--version").output().is_ok(),
            // TODO - figure out how to test docker install.
            NpmRunner::Docker => true,
        }
    }

    /// Runs the given NPM command given a chosen runner.
    fn run<I, S>(&self, dir: &Path, cmd: I) -> Result<ExitStatus, Box<dyn std::error::Error>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let result = match self {
            NpmRunner::NpmExec(npm) => Command::new(npm).args(cmd).current_dir(dir).status()?,
            NpmRunner::Docker => {
                Command::new("docker")
                .arg("run")
                .arg("--rm")
                .arg("-v")
                .arg(".:/app")
                .arg("-w")
                .arg("/app")
                // TODO - This version should get pulled from somewhere.
                .arg("node:lts-alpine")
                .arg("npm")
                .args(cmd)
                .current_dir(dir)
                .status()?},
        };
        Ok(result)
    }
}
