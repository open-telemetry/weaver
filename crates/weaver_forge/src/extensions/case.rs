// SPDX-License-Identifier: Apache-2.0

//! Case converter filters used by the template engine.

use crate::config::CaseConvention;
use minijinja::Environment;

/// Add case converter filters to the environment.
pub(crate) fn add_filters(env: &mut Environment<'_>) {
    env.add_filter("lower_case", case_converter(CaseConvention::LowerCase));
    env.add_filter("upper_case", case_converter(CaseConvention::UpperCase));
    env.add_filter("title_case", case_converter(CaseConvention::TitleCase));
    env.add_filter("pascal_case", case_converter(CaseConvention::PascalCase));
    env.add_filter("camel_case", case_converter(CaseConvention::CamelCase));
    env.add_filter("snake_case", case_converter(CaseConvention::SnakeCase));
    env.add_filter(
        "screaming_snake_case",
        case_converter(CaseConvention::ScreamingSnakeCase),
    );
    env.add_filter("kebab_case", case_converter(CaseConvention::KebabCase));
    env.add_filter(
        "screaming_kebab_case",
        case_converter(CaseConvention::ScreamingKebabCase),
    );
    env.add_filter("capitalize_first", capitalize_first);
}

/// Converts input string to the specified case convention.
#[must_use]
pub fn case_converter(case_convention: CaseConvention) -> fn(&str) -> String {
    match case_convention {
        CaseConvention::LowerCase => lower_case,
        CaseConvention::UpperCase => upper_case,
        CaseConvention::TitleCase => title_case,
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

/// Converts input string to title case
fn title_case(input: &str) -> String {
    CaseConvention::TitleCase.convert(input)
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

/// Capitalize the first character of a string.
fn capitalize_first(input: &str) -> String {
    let mut chars = input.chars();
    let mut result = String::with_capacity(input.len());

    if let Some(first_char) = chars.next() {
        for c in first_char.to_uppercase() {
            result.push(c);
        }
    }

    result.extend(chars);

    result
}

#[cfg(test)]
mod tests {
    use crate::extensions::case::add_filters;
    use minijinja::Environment;

    #[test]
    fn test_case_converter() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env);

        assert_eq!(
            env.render_str("{{ 'Hello World' | lower_case }}", &ctx)
                .unwrap(),
            "hello world"
        );
        assert_eq!(
            env.render_str("{{ 'Hello World' | upper_case }}", &ctx)
                .unwrap(),
            "HELLO WORLD"
        );
        assert_eq!(
            env.render_str("{{ 'Hello World' | title_case }}", &ctx)
                .unwrap(),
            "Hello World"
        );
        assert_eq!(
            env.render_str("{{ 'hello_world' | camel_case }}", &ctx)
                .unwrap(),
            "helloWorld"
        );
        assert_eq!(
            env.render_str("{{ 'hello_world' | pascal_case }}", &ctx)
                .unwrap(),
            "HelloWorld"
        );
        assert_eq!(
            env.render_str("{{ 'Hello World' | snake_case }}", &ctx)
                .unwrap(),
            "hello_world"
        );
        assert_eq!(
            env.render_str("{{ 'Hello World' | screaming_snake_case }}", &ctx)
                .unwrap(),
            "HELLO_WORLD"
        );
        assert_eq!(
            env.render_str("{{ 'Hello World' | kebab_case }}", &ctx)
                .unwrap(),
            "hello-world"
        );
        assert_eq!(
            env.render_str("{{ 'Hello World' | screaming_kebab_case }}", &ctx)
                .unwrap(),
            "HELLO-WORLD"
        );
        assert_eq!(
            env.render_str("{{ 'hello world' | capitalize_first }}", &ctx)
                .unwrap(),
            "Hello world"
        );
        assert_eq!(
            env.render_str("{{ 'Hello WORLD' | capitalize_first }}", &ctx)
                .unwrap(),
            "Hello WORLD"
        );
    }
}
