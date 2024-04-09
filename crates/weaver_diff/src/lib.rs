// SPDX-License-Identifier: Apache-2.0

//! This crate provides bare minimum support for colorized string differencing.

use similar::TextDiff;

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
