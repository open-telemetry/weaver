// SPDX-License-Identifier: Apache-2.0

use assert_cmd::Command;
use gix::clone::PrepareFetch;
use gix::create::{self, Kind};
use gix::{open, progress, Repository};
use semver::Version;
use std::fs;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const REPO_URL: &str = "https://github.com/open-telemetry/semantic-conventions.git";
const ARCHIVE_URL: &str =
    "https://github.com/open-telemetry/semantic-conventions/archive/refs/tags/";
const START_TAG: &str = "v1.25.0";
const TEMP_REPO_DIR: &str = "history-temp-repo";

/// Get the git repository, no checkout
#[cfg(not(tarpaulin_include))]
fn get_repo() -> anyhow::Result<Repository> {
    let _ = fs::remove_dir_all(TEMP_REPO_DIR);
    let _ = fs::create_dir(TEMP_REPO_DIR);
    let mut fetch = PrepareFetch::new(
        REPO_URL,
        TEMP_REPO_DIR,
        Kind::WithWorktree,
        create::Options {
            destination_must_be_empty: true,
            fs_capabilities: None,
        },
        open::Options::isolated(),
    )?;

    let (repo, _) = fetch.fetch_only(progress::Discard, &AtomicBool::new(false))?;
    Ok(repo)
}

/// Get the list of tags from the git repository filtered by the start version
#[cfg(not(tarpaulin_include))]
fn get_versions_from_git(repo: &Repository, start_semver: Version) -> anyhow::Result<Vec<String>> {
    let tags: Vec<String> = repo
        .references()?
        .tags()?
        .filter_map(|reference| {
            let reference = reference.ok()?;
            let tag = reference.name().shorten().to_string();

            // Ignore tags with a lower version than the start tag
            let version_str = tag.trim_start_matches(char::is_alphabetic);
            if let Ok(version) = Version::parse(version_str) {
                if version >= start_semver {
                    Some(tag)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    Ok(tags)
}

/// Run registry check on every semconv archive starting from start_version
#[cfg(not(tarpaulin_include))]
pub fn run(start_version: Option<String>) -> anyhow::Result<()> {
    use anyhow::Context;

    let start_version = start_version.unwrap_or(START_TAG.to_string());
    let start_semver = Version::parse(start_version.trim_start_matches(char::is_alphabetic))
        .context(format!(
            "The provided version `{start_version}` is not a valid semver."
        ))?;
    let repo = get_repo().context("Failed to fetch the semconv repo.")?;
    let versions =
        get_versions_from_git(&repo, start_semver).context("Failed to get the tag list.")?;
    let _ = fs::remove_dir_all(TEMP_REPO_DIR);
    println!("Checking versions: {:?}", versions);
    let mut failed = false;
    for version in versions {
        let mut cmd =
            Command::cargo_bin("weaver").context("Failed to create the cargo command.")?;
        let output = cmd
            .arg("--quiet")
            .arg("registry")
            .arg("check")
            .arg("-r")
            .arg(format!("{ARCHIVE_URL}{version}.zip[model]"))
            .timeout(Duration::from_secs(60))
            .output()
            .context("Failed to execute the weaver process.")?;

        if output.status.success() {
            println!("Success for: {}", version);
        } else {
            failed = true;
            println!("Failure for: {}", version);
            println!(
                "{}",
                String::from_utf8(output.stdout).context("Invalid UTF-8")?
            );
        }
    }
    if failed {
        anyhow::bail!("Some versions failed the registry check.");
    }
    Ok(())
}
