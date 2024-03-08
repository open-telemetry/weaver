// SPDX-License-Identifier: Apache-2.0

//! Case converter filters used by the template engine.

use crate::config::CaseConvention;

pub fn case_converter(case_convention: CaseConvention) -> fn(&str) -> String {
    match case_convention {
        CaseConvention::LowerCase => lower_case,
        CaseConvention::UpperCase => upper_case,
        CaseConvention::CamelCase => camel_case,
        CaseConvention::PascalCase => pascal_case,
        CaseConvention::SnakeCase => snake_case,
        CaseConvention::ScreamingSnakeCase => screaming_snake_case,
        CaseConvention::KebabCase => kebab_case,
        CaseConvention::ScreamingKebabCase => screaming_kebab_case,
    }
}

/// Converts input string to lower case
fn lower_case(input: &str) -> String {
    CaseConvention::LowerCase.convert(input)
}

/// Converts input string to upper case
fn upper_case(input: &str) -> String {
    CaseConvention::UpperCase.convert(input)
}

/// Converts input string to camel case
fn camel_case(input: &str) -> String {
    CaseConvention::CamelCase.convert(input)
}

/// Converts input string to pascal case
fn pascal_case(input: &str) -> String {
    CaseConvention::PascalCase.convert(input)
}

/// Converts input string to snake case
fn snake_case(input: &str) -> String {
    CaseConvention::SnakeCase.convert(input)
}

/// Converts input string to screaming snake case
fn screaming_snake_case(input: &str) -> String {
    CaseConvention::ScreamingSnakeCase.convert(input)
}

/// Converts input string to kebab case
fn kebab_case(input: &str) -> String {
    CaseConvention::KebabCase.convert(input)
}

/// Converts input string to screaming kebab case
fn screaming_kebab_case(input: &str) -> String {
    CaseConvention::ScreamingKebabCase.convert(input)
}
