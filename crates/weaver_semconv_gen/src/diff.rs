// SPDX-License-Identifier: Apache-2.0

//! Text diffing utilities.

const GREEN: &'static str = "\x1b[32m";
const RED: &'static str = "\x1b[31m";
const RESET: &'static str = "\x1b[0m";

// TODO - allow disabling colors
/// Constructs a "diff" string of the current snippet vs. updated on
/// outlining any changes that may need to be updated.
pub fn diff_output(original: &str, updated: &str) -> String {
    let mut result = String::new();
    let diff = 
        similar::TextDiff::from_lines(original, updated);
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