// SPDX-License-Identifier: Apache-2.0

//! Test that the UI is properly served by weaver serve command.

use std::io::Read;
use std::process::Command as StdCommand;
use std::thread::sleep;
use std::time::Duration;

/// Test that weaver serve starts and serves the UI correctly.
#[test]
fn test_ui_served() {
    // Start weaver serve command as a background process on a non-standard port
    let mut serve_cmd = StdCommand::new(env!("CARGO_BIN_EXE_weaver"))
        .args([
            "serve",
            "-r",
            "crates/weaver_emit/data",
            "--bind",
            "127.0.0.1:9080",
        ])
        .spawn()
        .expect("Failed to start weaver serve process");

    // Allow serve command to initialize
    sleep(Duration::from_secs(3));

    // Fetch index.html
    let index_response = ureq::get("http://localhost:9080/").call();

    assert!(
        index_response.is_ok(),
        "Failed to fetch index.html: {:?}",
        index_response.err()
    );

    let mut index_html = String::new();
    let _ = index_response
        .expect("Failed to get response")
        .into_body()
        .into_reader()
        .read_to_string(&mut index_html)
        .expect("Failed to read index.html as string");

    assert!(
        index_html.contains("<!DOCTYPE html>") || index_html.contains("<!doctype html>"),
        "index.html does not appear to be valid HTML"
    );

    // Verify the HTML contains references to the Svelte app (JS bundles)
    assert!(
        index_html.contains("/assets/")
            && (index_html.contains(".js") || index_html.contains(".css")),
        "index.html does not contain expected asset references"
    );

    // Kill the serve process
    serve_cmd
        .kill()
        .expect("Failed to kill weaver serve process");

    // Wait for it to terminate
    let _ = serve_cmd.wait();
}
