// SPDX-License-Identifier: Apache-2.0

//! Checks that generated Rust code and docs in `weaver_live_check` are up to
//! date with the model and templates. Fails if any generated file differs from
//! what the templates would produce.

use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;

const GENERATED_FILES: &[&str] = &[
    "crates/weaver_live_check/src/generated/attributes.rs",
    "crates/weaver_live_check/src/generated/events.rs",
    "crates/weaver_live_check/src/generated/entities.rs",
    "crates/weaver_live_check/docs/finding.md",
];

/// Locate the weaver binary, preferring release over debug.
#[cfg(not(tarpaulin_include))]
fn find_weaver_binary() -> anyhow::Result<PathBuf> {
    for profile in &["release", "debug"] {
        let path = PathBuf::from(format!("target/{profile}/weaver"));
        if path.exists() {
            return Ok(path);
        }
    }
    anyhow::bail!(
        "No weaver binary found in target/release or target/debug. Run 'cargo build' first."
    )
}

/// Run a command, printing its stdout/stderr and returning an error on failure.
#[cfg(not(tarpaulin_include))]
fn run_cmd(program: &str, args: &[&str], description: &str) -> anyhow::Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("Failed to execute: {description}"))?;
    if !status.success() {
        anyhow::bail!("{description} failed with exit code {status}");
    }
    Ok(())
}

/// Run the check-generated task.
#[cfg(not(tarpaulin_include))]
pub fn run() -> anyhow::Result<()> {
    let weaver = find_weaver_binary()?;
    let weaver = weaver
        .to_str()
        .context("Weaver binary path is not valid UTF-8")?;

    println!("Using weaver binary: {weaver}");

    println!("Regenerating Rust code...");
    run_cmd(
        weaver,
        &[
            "registry",
            "generate",
            "--registry",
            "crates/weaver_live_check/model/",
            "--templates",
            "crates/weaver_live_check/templates/",
            "--v2",
            "rust",
            "crates/weaver_live_check/src/",
        ],
        "Rust code generation",
    )?;

    println!("Regenerating documentation...");
    run_cmd(
        weaver,
        &[
            "registry",
            "generate",
            "--registry",
            "crates/weaver_live_check/model/",
            "--templates",
            "crates/weaver_live_check/templates/",
            "--v2",
            "markdown",
            "crates/weaver_live_check/docs/",
        ],
        "Markdown documentation generation",
    )?;

    println!("Formatting generated code...");
    run_cmd("cargo", &["fmt", "-p", "weaver_live_check"], "cargo fmt")?;

    println!("Checking for drift...");
    let diff_status = Command::new("git")
        .arg("diff")
        .arg("--exit-code")
        .arg("--")
        .args(GENERATED_FILES)
        .status()
        .context("Failed to run git diff")?;

    if !diff_status.success() {
        eprintln!();
        eprintln!("ERROR: Generated files are out of date with the model/templates.");
        eprintln!("Run the generate commands and commit the results.");
        #[allow(clippy::exit)]
        std::process::exit(1);
    }

    println!("All generated files are up to date.");
    Ok(())
}
