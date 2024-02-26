// SPDX-License-Identifier: Apache-2.0

//! Case converter filters used by the template engine.

use convert_case::{Case, Casing};

/// Converts input string to lower case
fn lower_case(input: String) -> String {
    input.to_case(Case::Lower)
}

/// Converts input string to upper case
fn upper_case(input: String) -> String {
    input.to_case(Case::Upper)
}

/// Converts input string to camel case
fn camel_case(input: String) -> String {
    input.to_case(Case::Camel)
}
