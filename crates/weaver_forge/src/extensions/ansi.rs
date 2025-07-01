// SPDX-License-Identifier: Apache-2.0

//! Set of filters used to add style and color to the console.

use minijinja::Value;

/// Converts the input value into a text with a black foreground color.
#[must_use]
pub(crate) fn black(input: &Value) -> String {
    format!("\x1b[30m{input}\x1b[0m")
}

/// Converts the input value into a text with a red foreground color.
#[must_use]
pub(crate) fn red(input: &Value) -> String {
    format!("\x1b[31m{input}\x1b[0m")
}

/// Converts the input value into a text with a green foreground color.
#[must_use]
pub(crate) fn green(input: &Value) -> String {
    format!("\x1b[32m{input}\x1b[0m")
}

/// Converts the input value into a text with a yellow foreground color.
#[must_use]
pub(crate) fn yellow(input: &Value) -> String {
    format!("\x1b[33m{input}\x1b[0m")
}

/// Converts the input value into a text with a blue foreground color.
#[must_use]
pub(crate) fn blue(input: &Value) -> String {
    format!("\x1b[34m{input}\x1b[0m")
}

/// Converts the input value into a text with a magenta foreground color.
#[must_use]
pub(crate) fn magenta(input: &Value) -> String {
    format!("\x1b[35m{input}\x1b[0m")
}

/// Converts the input value into a text with a cyan foreground color.
#[must_use]
pub(crate) fn cyan(input: &Value) -> String {
    format!("\x1b[36m{input}\x1b[0m")
}

/// Converts the input value into a text with a white foreground color.
#[must_use]
pub(crate) fn white(input: &Value) -> String {
    format!("\x1b[37m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright black foreground color.
#[must_use]
pub(crate) fn bright_black(input: &Value) -> String {
    format!("\x1b[90m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright red foreground color.
#[must_use]
pub(crate) fn bright_red(input: &Value) -> String {
    format!("\x1b[91m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright green foreground color.
#[must_use]
pub(crate) fn bright_green(input: &Value) -> String {
    format!("\x1b[92m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright yellow foreground color.
#[must_use]
pub(crate) fn bright_yellow(input: &Value) -> String {
    format!("\x1b[93m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright blue foreground color.
#[must_use]
pub(crate) fn bright_blue(input: &Value) -> String {
    format!("\x1b[94m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright magenta foreground color.
#[must_use]
pub(crate) fn bright_magenta(input: &Value) -> String {
    format!("\x1b[95m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright cyan foreground color.
#[must_use]
pub(crate) fn bright_cyan(input: &Value) -> String {
    format!("\x1b[96m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright white foreground color.
#[must_use]
pub(crate) fn bright_white(input: &Value) -> String {
    format!("\x1b[97m{input}\x1b[0m")
}

/// Converts the input value into a text with a black background color.
#[must_use]
pub(crate) fn bg_black(input: &Value) -> String {
    format!("\x1b[40m{input}\x1b[0m")
}

/// Converts the input value into a text with a red background color.
#[must_use]
pub(crate) fn bg_red(input: &Value) -> String {
    format!("\x1b[41m{input}\x1b[0m")
}

/// Converts the input value into a text with a green background color.
#[must_use]
pub(crate) fn bg_green(input: &Value) -> String {
    format!("\x1b[42m{input}\x1b[0m")
}

/// Converts the input value into a text with a yellow background color.
#[must_use]
pub(crate) fn bg_yellow(input: &Value) -> String {
    format!("\x1b[43m{input}\x1b[0m")
}

/// Converts the input value into a text with a blue background color.
#[must_use]
pub(crate) fn bg_blue(input: &Value) -> String {
    format!("\x1b[44m{input}\x1b[0m")
}

/// Converts the input value into a text with a magenta background color.
#[must_use]
pub(crate) fn bg_magenta(input: &Value) -> String {
    format!("\x1b[45m{input}\x1b[0m")
}

/// Converts the input value into a text with a cyan background color.
#[must_use]
pub(crate) fn bg_cyan(input: &Value) -> String {
    format!("\x1b[46m{input}\x1b[0m")
}

/// Converts the input value into a text with a white background color.
#[must_use]
pub(crate) fn bg_white(input: &Value) -> String {
    format!("\x1b[47m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright black background color.
#[must_use]
pub(crate) fn bg_bright_black(input: &Value) -> String {
    format!("\x1b[100m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright red background color.
#[must_use]
pub(crate) fn bg_bright_red(input: &Value) -> String {
    format!("\x1b[101m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright green background color.
#[must_use]
pub(crate) fn bg_bright_green(input: &Value) -> String {
    format!("\x1b[102m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright yellow background color.
#[must_use]
pub(crate) fn bg_bright_yellow(input: &Value) -> String {
    format!("\x1b[103m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright blue background color.
#[must_use]
pub(crate) fn bg_bright_blue(input: &Value) -> String {
    format!("\x1b[104m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright magenta background color.
#[must_use]
pub(crate) fn bg_bright_magenta(input: &Value) -> String {
    format!("\x1b[105m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright cyan background color.
#[must_use]
pub(crate) fn bg_bright_cyan(input: &Value) -> String {
    format!("\x1b[106m{input}\x1b[0m")
}

/// Converts the input value into a text with a bright white background color.
#[must_use]
pub(crate) fn bg_bright_white(input: &Value) -> String {
    format!("\x1b[107m{input}\x1b[0m")
}

/// Converts the input value into a text with a bold style.
#[must_use]
pub(crate) fn bold(input: &Value) -> String {
    format!("\x1b[1m{input}\x1b[0m")
}

/// Converts the input value into a text with an italic style.
#[must_use]
pub(crate) fn italic(input: &Value) -> String {
    format!("\x1b[3m{input}\x1b[0m")
}

/// Converts the input value into a text with an underline style.
#[must_use]
pub(crate) fn underline(input: &Value) -> String {
    format!("\x1b[4m{input}\x1b[0m")
}

/// Converts the input value into a text with a strikethrough style.
#[must_use]
pub(crate) fn strikethrough(input: &Value) -> String {
    format!("\x1b[9m{input}\x1b[0m")
}

/// Adds all the ANSI filters to the given environment.
pub(crate) fn add_filters(env: &mut minijinja::Environment<'_>) {
    env.add_filter("ansi_black", black);
    env.add_filter("ansi_red", red);
    env.add_filter("ansi_green", green);
    env.add_filter("ansi_yellow", yellow);
    env.add_filter("ansi_blue", blue);
    env.add_filter("ansi_magenta", magenta);
    env.add_filter("ansi_cyan", cyan);
    env.add_filter("ansi_white", white);

    env.add_filter("ansi_bright_black", bright_black);
    env.add_filter("ansi_bright_red", bright_red);
    env.add_filter("ansi_bright_green", bright_green);
    env.add_filter("ansi_bright_yellow", bright_yellow);
    env.add_filter("ansi_bright_blue", bright_blue);
    env.add_filter("ansi_bright_magenta", bright_magenta);
    env.add_filter("ansi_bright_cyan", bright_cyan);
    env.add_filter("ansi_bright_white", bright_white);

    env.add_filter("ansi_bg_black", bg_black);
    env.add_filter("ansi_bg_red", bg_red);
    env.add_filter("ansi_bg_green", bg_green);
    env.add_filter("ansi_bg_yellow", bg_yellow);
    env.add_filter("ansi_bg_blue", bg_blue);
    env.add_filter("ansi_bg_magenta", bg_magenta);
    env.add_filter("ansi_bg_cyan", bg_cyan);
    env.add_filter("ansi_bg_white", bg_white);

    env.add_filter("ansi_bg_bright_black", bg_bright_black);
    env.add_filter("ansi_bg_bright_red", bg_bright_red);
    env.add_filter("ansi_bg_bright_green", bg_bright_green);
    env.add_filter("ansi_bg_bright_yellow", bg_bright_yellow);
    env.add_filter("ansi_bg_bright_blue", bg_bright_blue);
    env.add_filter("ansi_bg_bright_magenta", bg_bright_magenta);
    env.add_filter("ansi_bg_bright_cyan", bg_bright_cyan);
    env.add_filter("ansi_bg_bright_white", bg_bright_white);

    env.add_filter("ansi_bold", bold);
    env.add_filter("ansi_italic", italic);
    env.add_filter("ansi_underline", underline);
    env.add_filter("ansi_strikethrough", strikethrough);
}

#[cfg(test)]
mod tests {
    use crate::extensions::ansi::add_filters;
    use minijinja::Environment;

    #[test]
    fn test_ansi_filters() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;

        add_filters(&mut env);

        assert_eq!(
            env.render_str("{{ 'test' | ansi_black }}", &ctx).unwrap(),
            "\x1b[30mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_red }}", &ctx).unwrap(),
            "\x1b[31mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_green }}", &ctx).unwrap(),
            "\x1b[32mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_yellow }}", &ctx).unwrap(),
            "\x1b[33mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_blue }}", &ctx).unwrap(),
            "\x1b[34mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_magenta }}", &ctx).unwrap(),
            "\x1b[35mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_cyan }}", &ctx).unwrap(),
            "\x1b[36mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_white }}", &ctx).unwrap(),
            "\x1b[37mtest\x1b[0m"
        );

        assert_eq!(
            env.render_str("{{ 'test' | ansi_bright_black }}", &ctx)
                .unwrap(),
            "\x1b[90mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bright_red }}", &ctx)
                .unwrap(),
            "\x1b[91mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bright_green }}", &ctx)
                .unwrap(),
            "\x1b[92mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bright_yellow }}", &ctx)
                .unwrap(),
            "\x1b[93mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bright_blue }}", &ctx)
                .unwrap(),
            "\x1b[94mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bright_magenta }}", &ctx)
                .unwrap(),
            "\x1b[95mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bright_cyan }}", &ctx)
                .unwrap(),
            "\x1b[96mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bright_white }}", &ctx)
                .unwrap(),
            "\x1b[97mtest\x1b[0m"
        );

        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_black }}", &ctx)
                .unwrap(),
            "\x1b[40mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_red }}", &ctx).unwrap(),
            "\x1b[41mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_green }}", &ctx)
                .unwrap(),
            "\x1b[42mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_yellow }}", &ctx)
                .unwrap(),
            "\x1b[43mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_blue }}", &ctx).unwrap(),
            "\x1b[44mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_magenta }}", &ctx)
                .unwrap(),
            "\x1b[45mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_cyan }}", &ctx).unwrap(),
            "\x1b[46mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_white }}", &ctx)
                .unwrap(),
            "\x1b[47mtest\x1b[0m"
        );

        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_bright_black }}", &ctx)
                .unwrap(),
            "\x1b[100mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_bright_red }}", &ctx)
                .unwrap(),
            "\x1b[101mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_bright_green }}", &ctx)
                .unwrap(),
            "\x1b[102mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_bright_yellow }}", &ctx)
                .unwrap(),
            "\x1b[103mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_bright_blue }}", &ctx)
                .unwrap(),
            "\x1b[104mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_bright_magenta }}", &ctx)
                .unwrap(),
            "\x1b[105mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_bright_cyan }}", &ctx)
                .unwrap(),
            "\x1b[106mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_bg_bright_white }}", &ctx)
                .unwrap(),
            "\x1b[107mtest\x1b[0m"
        );

        assert_eq!(
            env.render_str("{{ 'test' | ansi_bold }}", &ctx).unwrap(),
            "\x1b[1mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_italic }}", &ctx).unwrap(),
            "\x1b[3mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_underline }}", &ctx)
                .unwrap(),
            "\x1b[4mtest\x1b[0m"
        );
        assert_eq!(
            env.render_str("{{ 'test' | ansi_strikethrough }}", &ctx)
                .unwrap(),
            "\x1b[9mtest\x1b[0m"
        );

        assert_eq!(env.render_str("{{ 'test' | ansi_black | ansi_bg_white | ansi_bold | ansi_italic | ansi_underline | ansi_strikethrough }}", &ctx).unwrap(), "\u{1b}[9m\u{1b}[4m\u{1b}[3m\u{1b}[1m\u{1b}[47m\u{1b}[30mtest\u{1b}[0m\u{1b}[0m\u{1b}[0m\u{1b}[0m\u{1b}[0m\u{1b}[0m");
    }
}
