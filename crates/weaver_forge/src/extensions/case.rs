// SPDX-License-Identifier: Apache-2.0

//! Case converter filters used by the template engine.

use minijinja::Environment;

use crate::config::{CaseConvention, WeaverConfig};

/// Add case converter filters to the environment.
pub(crate) fn add_filters(env: &mut Environment<'_>, _: &WeaverConfig) {
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

/// Converts a `CaseConvention` to a function that converts a string to the specified case
/// convention.
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
pub(crate) fn lower_case(input: &str) -> String {
    CaseConvention::LowerCase.convert(input)
}

/// Converts input string to upper case
pub(crate) fn upper_case(input: &str) -> String {
    CaseConvention::UpperCase.convert(input)
}

/// Converts input string to title case
pub(crate) fn title_case(input: &str) -> String {
    CaseConvention::TitleCase.convert(input)
}

/// Converts input string to camel case
pub(crate) fn camel_case(input: &str) -> String {
    CaseConvention::CamelCase.convert(input)
}

/// Converts input string to pascal case
pub(crate) fn pascal_case(input: &str) -> String {
    CaseConvention::PascalCase.convert(input)
}

/// Converts input string to snake case
pub(crate) fn snake_case(input: &str) -> String {
    CaseConvention::SnakeCase.convert(input)
}

/// Converts input string to screaming snake case
pub(crate) fn screaming_snake_case(input: &str) -> String {
    CaseConvention::ScreamingSnakeCase.convert(input)
}

/// Converts input string to kebab case
pub(crate) fn kebab_case(input: &str) -> String {
    CaseConvention::KebabCase.convert(input)
}

/// Converts input string to screaming kebab case
pub(crate) fn screaming_kebab_case(input: &str) -> String {
    CaseConvention::ScreamingKebabCase.convert(input)
}

/// Capitalize the first character of a string.
pub(crate) fn capitalize_first(input: &str) -> String {
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
    use minijinja::Environment;

    use crate::config::WeaverConfig;
    use crate::extensions::case::add_filters;

    #[test]
    fn test_kebab_case() {
        let target_config = WeaverConfig::default();
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env, &target_config);


        assert_eq!(
            env.render_str("{{ 'Hello World' | kebab_case }}", &ctx)
                .unwrap(),
            "hello-world"
        );
    }

    #[test]
    fn test_lower_case() {
        let target_config = WeaverConfig::default();
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env, &target_config);

        assert_eq!(
            env.render_str("{{ 'Hello World' | lower_case }}", &ctx)
                .unwrap(),
            "hello world"
        );
    }

    #[test]
    fn test_upper_case() {
        let target_config = WeaverConfig::default();
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env, &target_config);

        assert_eq!(
            env.render_str("{{ 'Hello World' | upper_case }}", &ctx)
                .unwrap(),
            "HELLO WORLD"
        );
    }

    #[test]
    fn test_title_case() {
        let target_config = WeaverConfig::default();
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env, &target_config);

        assert_eq!(
            env.render_str("{{ 'Hello World' | title_case }}", &ctx)
                .unwrap(),
            "Hello World"
        );
    }

    #[test]
    fn test_camel_case() {
        let target_config = WeaverConfig::default();
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env, &target_config);

        assert_eq!(
            env.render_str("{{ 'hello_world' | camel_case }}", &ctx)
                .unwrap(),
            "helloWorld"
        );
    }

    #[test]
    fn test_pascal_case() {
        let target_config = WeaverConfig::default();
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env, &target_config);

        assert_eq!(
            env.render_str("{{ 'hello_world' | pascal_case }}", &ctx)
                .unwrap(),
            "HelloWorld"
        );
    }

    #[test]
    fn test_capitalize_first_case() {
        let target_config = WeaverConfig::default();
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env, &target_config);

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

    #[test]
    fn test_screaming_snake_case() {
        let target_config = WeaverConfig::default();
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env, &target_config);

        assert_eq!(
            env.render_str("{{ 'Hello World' | screaming_snake_case }}", &ctx)
                .unwrap(),
            "HELLO_WORLD"
        );
    }

    #[test]
    fn test_screaming_kebab_case() {
        let target_config = WeaverConfig::default();
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env, &target_config);

        assert_eq!(
            env.render_str("{{ 'Hello World' | screaming_kebab_case }}", &ctx)
                .unwrap(),
            "HELLO-WORLD"
        );
    }

    #[test]
    fn test_snake_case() {
        let target_config = WeaverConfig::default();
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env, &target_config);

        assert_eq!(
            env.render_str("{{ 'v8js.heap.space.name' | snake_case }}", &ctx)
                .unwrap(),
            "v8js_heap_space_name"
        );

        assert_eq!(
            env.render_str("{{ 'k8s.job.name' | snake_case }}", &ctx)
                .unwrap(),
            "k8s_job_name"
        );

        assert_eq!(
            env.render_str("{{ 'host.cpu.cache.l2.size' | snake_case }}", &ctx)
                .unwrap(),
            "host_cpu_cache_l2_size"
        );

        assert_eq!(
            env.render_str("{{ 'nodejs.eventloop.delay.p99' | snake_case }}", &ctx)
                .unwrap(),
            "nodejs_eventloop_delay_p99"
        );

        assert_eq!(
            env.render_str("{{ 'http.request.resend_count' | snake_case }}", &ctx)
                .unwrap(),
            "http_request_resend_count"
        );

        assert_eq!(
            env.render_str("{{ 'Hello World!' | snake_case }}", &ctx)
                .unwrap(),
            "hello_world!"
        );

        assert_eq!(
            env.render_str("{{ 'this IS an ios device with a nice api!' | snake_case }}", &ctx)
                .unwrap(),
            "this_is_an_ios_device_with_a_nice_api!"
        );
    }
}
