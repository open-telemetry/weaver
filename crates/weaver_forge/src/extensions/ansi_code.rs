// SPDX-License-Identifier: Apache-2.0

//! Set of filters used to add style and color to the console.

use minijinja::Value;

/// Converts the input value into a text with a black foreground color.
#[must_use]
pub(crate) fn black(input: &Value) -> String {
    format!("\x1b[30m{}\x1b[0m", input)
}

/// Converts the input value into a text with a red foreground color.
#[must_use]
pub(crate) fn red(input: &Value) -> String {
    format!("\x1b[31m{}\x1b[0m", input)
}

/// Converts the input value into a text with a green foreground color.
#[must_use]
pub(crate) fn green(input: &Value) -> String {
    format!("\x1b[32m{}\x1b[0m", input)
}

/// Converts the input value into a text with a yellow foreground color.
#[must_use]
pub(crate) fn yellow(input: &Value) -> String {
    format!("\x1b[33m{}\x1b[0m", input)
}

/// Converts the input value into a text with a blue foreground color.
#[must_use]
pub(crate) fn blue(input: &Value) -> String {
    format!("\x1b[34m{}\x1b[0m", input)
}

/// Converts the input value into a text with a magenta foreground color.
#[must_use]
pub(crate) fn magenta(input: &Value) -> String {
    format!("\x1b[35m{}\x1b[0m", input)
}

/// Converts the input value into a text with a cyan foreground color.
#[must_use]
pub(crate) fn cyan(input: &Value) -> String {
    format!("\x1b[36m{}\x1b[0m", input)
}

/// Converts the input value into a text with a white foreground color.
#[must_use]
pub(crate) fn white(input: &Value) -> String {
    format!("\x1b[37m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright black foreground color.
#[must_use]
pub(crate) fn bright_black(input: &Value) -> String {
    format!("\x1b[90m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright red foreground color.
#[must_use]
pub(crate) fn bright_red(input: &Value) -> String {
    format!("\x1b[91m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright green foreground color.
#[must_use]
pub(crate) fn bright_green(input: &Value) -> String {
    format!("\x1b[92m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright yellow foreground color.
#[must_use]
pub(crate) fn bright_yellow(input: &Value) -> String {
    format!("\x1b[93m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright blue foreground color.
#[must_use]
pub(crate) fn bright_blue(input: &Value) -> String {
    format!("\x1b[94m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright magenta foreground color.
#[must_use]
pub(crate) fn bright_magenta(input: &Value) -> String {
    format!("\x1b[95m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright cyan foreground color.
#[must_use]
pub(crate) fn bright_cyan(input: &Value) -> String {
    format!("\x1b[96m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright white foreground color.
#[must_use]
pub(crate) fn bright_white(input: &Value) -> String {
    format!("\x1b[97m{}\x1b[0m", input)
}

/// Converts the input value into a text with a black background color.
#[must_use]
pub(crate) fn bg_black(input: &Value) -> String {
    format!("\x1b[40m{}\x1b[0m", input)
}

/// Converts the input value into a text with a red background color.
#[must_use]
pub(crate) fn bg_red(input: &Value) -> String {
    format!("\x1b[41m{}\x1b[0m", input)
}

/// Converts the input value into a text with a green background color.
#[must_use]
pub(crate) fn bg_green(input: &Value) -> String {
    format!("\x1b[42m{}\x1b[0m", input)
}

/// Converts the input value into a text with a yellow background color.
#[must_use]
pub(crate) fn bg_yellow(input: &Value) -> String {
    format!("\x1b[43m{}\x1b[0m", input)
}

/// Converts the input value into a text with a blue background color.
#[must_use]
pub(crate) fn bg_blue(input: &Value) -> String {
    format!("\x1b[44m{}\x1b[0m", input)
}

/// Converts the input value into a text with a magenta background color.
#[must_use]
pub(crate) fn bg_magenta(input: &Value) -> String {
    format!("\x1b[45m{}\x1b[0m", input)
}

/// Converts the input value into a text with a cyan background color.
#[must_use]
pub(crate) fn bg_cyan(input: &Value) -> String {
    format!("\x1b[46m{}\x1b[0m", input)
}

/// Converts the input value into a text with a white background color.
#[must_use]
pub(crate) fn bg_white(input: &Value) -> String {
    format!("\x1b[47m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright black background color.
#[must_use]
pub(crate) fn bg_bright_black(input: &Value) -> String {
    format!("\x1b[100m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright red background color.
#[must_use]
pub(crate) fn bg_bright_red(input: &Value) -> String {
    format!("\x1b[101m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright green background color.
#[must_use]
pub(crate) fn bg_bright_green(input: &Value) -> String {
    format!("\x1b[102m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright yellow background color.
#[must_use]
pub(crate) fn bg_bright_yellow(input: &Value) -> String {
    format!("\x1b[103m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright blue background color.
#[must_use]
pub(crate) fn bg_bright_blue(input: &Value) -> String {
    format!("\x1b[104m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright magenta background color.
#[must_use]
pub(crate) fn bg_bright_magenta(input: &Value) -> String {
    format!("\x1b[105m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright cyan background color.
#[must_use]
pub(crate) fn bg_bright_cyan(input: &Value) -> String {
    format!("\x1b[106m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bright white background color.
#[must_use]
pub(crate) fn bg_bright_white(input: &Value) -> String {
    format!("\x1b[107m{}\x1b[0m", input)
}

/// Converts the input value into a text with a bold style.
#[must_use]
pub(crate) fn bold(input: &Value) -> String {
    format!("\x1b[1m{}\x1b[0m", input)
}

/// Converts the input value into a text with an italic style.
#[must_use]
pub(crate) fn italic(input: &Value) -> String {
    format!("\x1b[3m{}\x1b[0m", input)
}

/// Converts the input value into a text with an underline style.
#[must_use]
pub(crate) fn underline(input: &Value) -> String {
    format!("\x1b[4m{}\x1b[0m", input)
}

/// Converts the input value into a text with a strikethrough style.
#[must_use]
pub(crate) fn strikethrough(input: &Value) -> String {
    format!("\x1b[9m{}\x1b[0m", input)
}
