//! See <https://github.com/matklad/cargo-xtask/>.
//!
//! This binary defines various auxiliary build commands, which are not
//! expressible with just `cargo`.
//!
//! This binary is integrated into the `cargo` command line by using an alias in
//! `.cargo/config`.

// This crate is a CLI tool and can use stdout and stderr for logging.
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

mod downstream;
mod history;
mod patch_release_workflow;
mod schema_compat;
mod validate;

#[cfg(not(tarpaulin_include))]
fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("Failed to install the ring Rustls crypto provider"))?;

    let task = std::env::args().nth(1);

    match task {
        None => print_help(),
        Some(task) => match task.as_str() {
            "patch-release-workflow" => patch_release_workflow::run(),
            "validate" => validate::run(),
            "history" => history::run(std::env::args().nth(2)),
            "schema-compat" => schema_compat::run(),
            "downstream-check" => downstream::run(std::env::args().skip(2).collect()),
            "help" => print_help(),
            _ => {
                eprintln!("Unknown task: {task}");
                print_help()
            }
        },
    }
}

/// Prints help message.
#[cfg(not(tarpaulin_include))]
pub fn print_help() -> anyhow::Result<()> {
    println!(
        "
Usage: Execute the command using `cargo xtask <task>`, e.g., `cargo xtask validate`.

Tasks:
  - patch-release-workflow: Patch release.yml after `dist generate` (permissions, scorecard shas, smoke tests).
  - validate: Validate the entire structure of the weaver project.
  - history: Run registry check on semconv models within back compatibility range.
             Optionally provide a start semver e.g. `history 1.29.0`.
  - schema-compat: Check JSON schema backwards and forwards compatibility against the latest release.
  - downstream-check: Run the weaver from this working tree against the CI checks of the downstream
                      repos declared in downstream-check.yaml. Optionally provide the repos to check
                      as `<url>[@<ref>]`, e.g.
                      `downstream-check https://github.com/open-telemetry/semantic-conventions.git@main`.
                      No repos = all of them.
"
    );
    Ok(())
}
