// SPDX-License-Identifier: Apache-2.0

//! This crate provides bare minimum support for colorized string differencing.

use similar::TextDiff;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

/// Constructs a "diff" string of the original vs. updated.
/// Will create colorized (ANSI) output w/ `+` representing additions and `-` representing removals.
#[must_use]
pub fn diff_output(original: &str, updated: &str) -> String {
    let mut result = String::new();
    let diff = TextDiff::from_lines(original, updated);
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            similar::ChangeTag::Delete => "-",
            similar::ChangeTag::Insert => "+",
            similar::ChangeTag::Equal => " ",
        };
        let color = match change.tag() {
            similar::ChangeTag::Delete => RED,
            similar::ChangeTag::Insert => GREEN,
            similar::ChangeTag::Equal => RESET,
        };
        result.push_str(&format!("{}{} {}", color, sign, change));
    }
    result.push_str(RESET);
    result
}

/// Displays differences between two directories and returns whether they are identical.
/// The function will print differences to stderr.
#[allow(clippy::print_stderr)]
pub fn diff_dir<P: AsRef<Path>>(expected_dir: P, observed_dir: P) -> std::io::Result<bool> {
    let mut expected_files = HashSet::new();
    let mut observed_files = HashSet::new();

    // Walk through the first directory and add files to files1 set
    for entry in WalkDir::new(&expected_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            let relative_path = path
                .strip_prefix(&expected_dir)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            _ = expected_files.insert(relative_path.to_path_buf());
        }
    }

    // Walk through the second directory and add files to files2 set
    for entry in WalkDir::new(&observed_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            let relative_path = path
                .strip_prefix(&observed_dir)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            _ = observed_files.insert(relative_path.to_path_buf());
        }
    }

    // Assume directories are identical until proven otherwise
    let mut are_identical = true;

    // Compare files in both sets
    for file in expected_files.intersection(&observed_files) {
        let file1_content =
            fs::read_to_string(expected_dir.as_ref().join(file))?.replace("\r\n", "\n");
        let file2_content =
            fs::read_to_string(observed_dir.as_ref().join(file))?.replace("\r\n", "\n");

        if file1_content != file2_content {
            are_identical = false;
            eprintln!(
                "Files {:?} and {:?} are different",
                expected_dir.as_ref().join(file),
                observed_dir.as_ref().join(file)
            );

            eprintln!(
                "Found differences:\n{}",
                diff_output(&file1_content, &file2_content)
            );
            break;
        }
    }
    // If any file is unique to one directory, they are not identical
    let not_in_observed = expected_files
        .difference(&observed_files)
        .collect::<Vec<_>>();
    if !not_in_observed.is_empty() {
        are_identical = false;
        eprintln!("Observed output is missing files: {:?}", not_in_observed);
    }
    let not_in_expected = observed_files
        .difference(&expected_files)
        .collect::<Vec<_>>();
    if !not_in_expected.is_empty() {
        are_identical = false;
        eprintln!(
            "Observed output has unexpected files: {:?}",
            not_in_expected
        );
    }

    Ok(are_identical)
}
