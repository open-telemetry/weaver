// SPDX-License-Identifier: Apache-2.0

//! Runs the weaver built from this working tree against the CI checks of the
//! downstream repositories that consume weaver.
//!
//! The repos and the checks to run in each of them are declared in
//! `downstream-check.yaml` at the workspace root; the command line selects
//! which of them to run.

use anyhow::{bail, Context};
use serde::Deserialize;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Declares the downstream repos and their checks.
const CONFIG_FILE: &str = "downstream-check.yaml";
/// Ref to check out when neither the config nor the command line pins one.
const DEFAULT_REF: &str = "main";
/// Where the downstream repos are cloned, relative to the workspace root.
const CHECKOUT_DIR: &str = "target/downstream";
/// Tag of the locally built image, used by repos that only run weaver in Docker.
const LOCAL_IMAGE: &str = "weaver-downstream-check:local";
/// Wall clock a single check gets before it is killed, unless it sets its own.
const DEFAULT_TIMEOUT_MINUTES: u64 = 20;
/// How often a running check is polled for completion.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    repos: Vec<Repo>,
}

/// Repos that have no way to run a local weaver binary, so the checks must run
/// against a container image built from this working tree instead.
///
/// TODO: teach semantic-conventions to use a local weaver when one is present,
/// the way semantic-conventions-genai does, and drop this special case.
const IMAGE_ONLY_REPOS: &[&str] = &["github.com/open-telemetry/semantic-conventions"];

/// How weaver is handed to a downstream repo.
#[derive(PartialEq, Eq, Clone, Copy)]
enum WeaverKind {
    /// The repo invokes a `weaver` binary; `PATH` and `$WEAVER` point at ours.
    Binary,
    /// The repo only knows how to run weaver as a container image.
    Image,
}

/// A downstream repository and the checks to run against it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Repo {
    /// Clone URL, optionally suffixed with `@<ref>`. Also how the repo is named
    /// on the command line, where the ref suffix overrides the one configured here.
    url: String,
    checks: Vec<Check>,
}

impl Repo {
    fn clone_url(&self) -> &str {
        split_ref(&self.url).0
    }

    fn git_ref(&self) -> &str {
        split_ref(&self.url).1.unwrap_or(DEFAULT_REF)
    }

    fn weaver(&self) -> WeaverKind {
        let url = normalize_url(self.clone_url());
        if IMAGE_ONLY_REPOS.iter().any(|r| normalize_url(r) == url) {
            WeaverKind::Image
        } else {
            WeaverKind::Binary
        }
    }
}

/// Strips the scheme and the `.git` suffix so URLs can be compared.
fn normalize_url(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .trim_end_matches('/')
        .to_owned()
}

/// Splits `<url>[@<ref>]`. Only an `@` after the last `/` separates a ref, so
/// scp-style URLs (`git@github.com:org/repo`) are left alone.
fn split_ref(url_and_ref: &str) -> (&str, Option<&str>) {
    match url_and_ref.rfind('@') {
        Some(i) if !url_and_ref[i..].contains('/') => {
            (&url_and_ref[..i], Some(&url_and_ref[i + 1..]))
        }
        _ => (url_and_ref, None),
    }
}

/// One command to run in a checked-out downstream repo.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Check {
    name: String,
    /// Working directory, relative to the repo root.
    #[serde(default = "default_dir")]
    dir: String,
    /// Shell command. `{weaver}` expands to the weaver binary path or image tag.
    /// Run through `sh -c`, so it can chain a generation step with the
    /// `git diff` that asserts the committed output is in sync.
    run: String,
    /// Overrides `DEFAULT_TIMEOUT_MINUTES` for checks that legitimately take longer.
    timeout_minutes: Option<u64>,
}

fn default_dir() -> String {
    ".".to_owned()
}

/// Outcome of a single downstream check.
#[derive(PartialEq, Eq)]
enum Status {
    Ok,
    Failed,
}

impl Status {
    fn label(&self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::Failed => "FAILED",
        }
    }
}

struct Outcome {
    repo: String,
    check: String,
    status: Status,
    detail: String,
}

/// A repo selected on the command line, with the ref to check out resolved.
struct Selection<'a> {
    repo: &'a Repo,
    git_ref: String,
}

impl Selection<'_> {
    /// `<url>@<ref>`, as printed in the output and accepted on the command line.
    fn id(&self) -> String {
        format!("{}@{}", self.repo.clone_url(), self.git_ref)
    }

    /// Directory to check the repo out into, e.g. `open-telemetry_semantic-conventions`.
    fn dir_name(&self) -> String {
        let mut parts: Vec<_> = self
            .repo
            .clone_url()
            .trim_end_matches(".git")
            .rsplit('/')
            .take(2)
            .collect();
        parts.reverse();
        parts.join("_")
    }
}

/// Runs the downstream checks. Each argument is the `<url>[@<ref>]` of a repo
/// declared in the config; with no arguments, every repo runs at its configured ref.
#[cfg(not(tarpaulin_include))]
pub fn run(repos: Vec<String>) -> anyhow::Result<()> {
    let config: Config = serde_yaml::from_str(
        &std::fs::read_to_string(CONFIG_FILE)
            .with_context(|| format!("Failed to read {CONFIG_FILE}"))?,
    )
    .with_context(|| format!("Failed to parse {CONFIG_FILE}"))?;

    let selected: Vec<Selection<'_>> = if repos.is_empty() {
        config
            .repos
            .iter()
            .map(|repo| Selection {
                repo,
                git_ref: repo.git_ref().to_owned(),
            })
            .collect()
    } else {
        repos
            .iter()
            .map(|arg| select(&config, arg))
            .collect::<anyhow::Result<_>>()?
    };

    let checkout_root = PathBuf::from(CHECKOUT_DIR);
    std::fs::create_dir_all(&checkout_root)?;

    let needs_binary = selected
        .iter()
        .any(|s| s.repo.weaver() == WeaverKind::Binary);
    let needs_image = selected
        .iter()
        .any(|s| s.repo.weaver() == WeaverKind::Image);
    let weaver_bin = needs_binary.then(build_weaver_binary).transpose()?;
    let weaver_image = needs_image.then(build_weaver_image).transpose()?;

    let mut outcomes = Vec::new();
    for selection in &selected {
        println!("\n{:=<70}\n=== {}\n{:=<70}", "", selection.id(), "");
        let dir = checkout_root.join(selection.dir_name());
        if let Err(e) = checkout(selection, &dir) {
            outcomes.push(Outcome {
                repo: selection.id(),
                check: "checkout".to_owned(),
                status: Status::Failed,
                detail: format!("{e:#}"),
            });
            continue;
        }

        // Repos that call `weaver` directly, or that read $WEAVER, must see ours.
        // Never fall back to a `weaver` on PATH or a published image: that would
        // silently check a released weaver and report it as a pass.
        let bin = match selection.repo.weaver() {
            WeaverKind::Binary => Some(
                weaver_bin
                    .as_deref()
                    .context("internal error: no weaver binary was built")?,
            ),
            WeaverKind::Image => None,
        };
        let weaver = match bin {
            Some(bin) => bin.display().to_string(),
            None => weaver_image
                .as_deref()
                .context("internal error: no weaver image was built")?
                .to_owned(),
        };

        for check in &selection.repo.checks {
            let command = check.run.replace("{weaver}", &weaver);
            outcomes.push(run_check(
                selection,
                check,
                &dir.join(&check.dir),
                &command,
                bin,
            ));
        }
    }

    report(&outcomes)
}

/// Resolves a `<url>[@<ref>]` argument against the configured repos. The URL may
/// omit the scheme and the `.git` suffix.
#[cfg(not(tarpaulin_include))]
fn select<'a>(config: &'a Config, arg: &str) -> anyhow::Result<Selection<'a>> {
    let (url, git_ref) = split_ref(arg);

    let wanted = normalize_url(url);
    let repo = config
        .repos
        .iter()
        .find(|r| normalize_url(r.clone_url()) == wanted)
        .with_context(|| {
            let known: Vec<_> = config.repos.iter().map(|r| r.url.as_str()).collect();
            format!(
                "Unknown repo `{url}`. Repos declared in {CONFIG_FILE}:\n  {}",
                known.join("\n  ")
            )
        })?;

    Ok(Selection {
        repo,
        git_ref: git_ref.unwrap_or_else(|| repo.git_ref()).to_owned(),
    })
}

/// Builds weaver from this working tree and returns the absolute binary path.
#[cfg(not(tarpaulin_include))]
fn build_weaver_binary() -> anyhow::Result<PathBuf> {
    if let Ok(bin) = std::env::var("WEAVER_BIN") {
        println!("=== Using weaver binary from WEAVER_BIN: {bin}");
        return Ok(PathBuf::from(bin));
    }
    println!("=== Building weaver (cargo build --release)");
    let status = Command::new("cargo")
        .args(["build", "--release", "--locked", "--bin", "weaver"])
        .status()?;
    if !status.success() {
        bail!("Failed to build weaver: {status}");
    }
    let bin = std::fs::canonicalize("target/release/weaver")
        .context("weaver binary not found after build")?;
    println!("=== weaver under test: {}", bin.display());
    Ok(bin)
}

/// Builds the weaver container image from this working tree and returns its tag.
#[cfg(not(tarpaulin_include))]
fn build_weaver_image() -> anyhow::Result<String> {
    if let Ok(image) = std::env::var("WEAVER_IMAGE") {
        println!("=== Using weaver image from WEAVER_IMAGE: {image}");
        return Ok(image);
    }
    println!("=== Building {LOCAL_IMAGE} (docker build)");
    let status = Command::new("docker")
        .args(["build", "-t", LOCAL_IMAGE, "."])
        .status()
        .context("Failed to run `docker build`. Is docker installed and running?")?;
    if !status.success() {
        bail!("Failed to build the weaver image: {status}");
    }
    Ok(LOCAL_IMAGE.to_owned())
}

/// Checks the repo out at the selected ref, reusing an existing checkout.
#[cfg(not(tarpaulin_include))]
fn checkout(selection: &Selection<'_>, dir: &Path) -> anyhow::Result<()> {
    let git = |args: &[&str]| -> anyhow::Result<()> {
        let status = Command::new("git").args(args).status()?;
        if !status.success() {
            bail!("`git {}` failed: {status}", args.join(" "));
        }
        Ok(())
    };
    let dir = dir.display().to_string();
    println!("=== Checking out {} into {dir}", selection.id());
    if !Path::new(&dir).join(".git").exists() {
        std::fs::create_dir_all(&dir)?;
        git(&["-C", &dir, "init", "--quiet"])?;
        git(&[
            "-C",
            &dir,
            "remote",
            "add",
            "origin",
            selection.repo.clone_url(),
        ])?;
    }
    // Fetching the ref explicitly works for branches, tags and commit shas alike.
    git(&[
        "-C",
        &dir,
        "fetch",
        "--quiet",
        "--depth",
        "1",
        "origin",
        &selection.git_ref,
    ])?;
    git(&["-C", &dir, "reset", "--quiet", "--hard", "FETCH_HEAD"])?;
    git(&["-C", &dir, "clean", "-qfdx"])?;
    Ok(())
}

/// Runs one check, streaming its output, and classifies the result.
#[cfg(not(tarpaulin_include))]
fn run_check(
    selection: &Selection<'_>,
    check: &Check,
    workdir: &Path,
    command: &str,
    weaver_bin: Option<&Path>,
) -> Outcome {
    let title = format!("{}: {}", selection.repo.clone_url(), check.name);
    group_start(&title);
    println!("--- in {}", workdir.display());
    println!("--- $ {command}");

    let mut cmd = Command::new("sh");
    let _ = cmd
        .arg("-c")
        .arg(command)
        .current_dir(workdir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    // Own process group, so a timeout can take the whole check down: several of
    // them background a long-running weaver that would outlive the `sh` alone.
    #[cfg(unix)]
    let _ = std::os::unix::process::CommandExt::process_group(&mut cmd, 0);

    if let Some(bin) = weaver_bin {
        let _ = cmd.env("WEAVER", bin);
        if let Some(bin_dir) = bin.parent() {
            let path = std::env::var("PATH").unwrap_or_default();
            let _ = cmd.env("PATH", format!("{}:{path}", bin_dir.display()));
        }
    }

    let timeout =
        Duration::from_secs(check.timeout_minutes.unwrap_or(DEFAULT_TIMEOUT_MINUTES) * 60);
    let result = cmd
        .spawn()
        .and_then(|child| wait_with_timeout(child, timeout));
    group_end();

    let (status, detail) = match result {
        Err(e) => (Status::Failed, format!("could not start: {e}")),
        Ok(None) => (
            Status::Failed,
            format!("timed out after {} minutes", timeout.as_secs() / 60),
        ),
        Ok(Some(exit)) => match exit.code() {
            Some(0) => (Status::Ok, String::new()),
            Some(code) => (Status::Failed, format!("exit {code}")),
            // No exit code means the process was killed by a signal.
            None => (Status::Failed, "killed by signal".to_owned()),
        },
    };

    if status != Status::Ok {
        eprintln!("!!! [{title}] {} ({detail})", status.label());
        if std::env::var("GITHUB_ACTIONS").is_ok() {
            println!(
                "::error title={title}::{} ({detail}) - see the '{title}' group above",
                status.label()
            );
        }
    }

    Outcome {
        repo: selection.id(),
        check: check.name.clone(),
        status,
        detail,
    }
}

/// Waits for a check to finish, returning `None` if it had to be killed on timeout.
#[cfg(not(tarpaulin_include))]
fn wait_with_timeout(
    mut child: Child,
    timeout: Duration,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(exit) = child.try_wait()? {
            return Ok(Some(exit));
        }
        if Instant::now() >= deadline {
            eprintln!("!!! timed out, killing the check");
            kill_check(&mut child);
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Signals the whole process group of a check, so anything it spawned dies too.
#[cfg(all(unix, not(tarpaulin_include)))]
fn kill_check(child: &mut Child) {
    let pid = child.id();
    for signal in ["-TERM", "-KILL"] {
        let _ = Command::new("kill")
            .args([signal, &format!("-{pid}")])
            .status();
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// The checks are `sh` commands, so this task only really runs on unix; keep it
/// compiling elsewhere by killing just the child we spawned.
#[cfg(all(not(unix), not(tarpaulin_include)))]
fn kill_check(child: &mut Child) {
    let _ = child.kill();
}

/// Prints the summary and fails if any check did not pass.
#[cfg(not(tarpaulin_include))]
fn report(outcomes: &[Outcome]) -> anyhow::Result<()> {
    let mut summary = String::new();
    for o in outcomes {
        let _ = writeln!(
            summary,
            "{:<9} {}: {} {}",
            o.status.label(),
            o.repo,
            o.check,
            o.detail
        );
    }
    println!("\n{:=<70}\n=== Summary\n{:=<70}\n{summary}", "", "");
    write_step_summary(outcomes);

    if outcomes.iter().any(|o| o.status != Status::Ok) {
        bail!("Some downstream checks did not pass; see the output above.");
    }
    println!("All downstream checks passed.");
    Ok(())
}

/// Appends the summary as a markdown table to the GitHub job summary, so a failed
/// run says which repo and check broke without opening the logs.
#[cfg(not(tarpaulin_include))]
fn write_step_summary(outcomes: &[Outcome]) {
    let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY") else {
        return;
    };
    let mut md = String::from("| | Repo | Check | Detail |\n|---|---|---|---|\n");
    for o in outcomes {
        let icon = if o.status == Status::Ok { "✅" } else { "❌" };
        let _ = writeln!(md, "| {icon} | {} | {} | {} |", o.repo, o.check, o.detail);
    }
    if let Err(e) = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, md.as_bytes()))
    {
        eprintln!("Failed to write {path}: {e}");
    }
}

/// Collapses a check's output into a foldable group when running on GitHub.
#[cfg(not(tarpaulin_include))]
fn group_start(title: &str) {
    if std::env::var("GITHUB_ACTIONS").is_ok() {
        println!("::group::{title}");
    } else {
        println!("\n--- [{title}]");
    }
}

#[cfg(not(tarpaulin_include))]
fn group_end() {
    if std::env::var("GITHUB_ACTIONS").is_ok() {
        println!("::endgroup::");
    }
}
