// SPDX-License-Identifier: Apache-2.0

//! The namespace separator used in registry names, set once for the process.

use std::sync::OnceLock;

/// The separator used unless one is configured.
const DEFAULT_NAMESPACE_SEPARATOR: &str = ".";

static NAMESPACE_SEPARATOR: OnceLock<String> = OnceLock::new();

/// Set the namespace separator used in registry names. Takes effect once per
/// process; later calls are ignored.
pub fn set_namespace_separator(separator: String) {
    let _ = NAMESPACE_SEPARATOR.set(separator);
}

/// The namespace separator used in registry names, such as the `.` in
/// `http.request.method`.
#[must_use]
pub fn namespace_separator() -> &'static str {
    NAMESPACE_SEPARATOR
        .get()
        .map_or(DEFAULT_NAMESPACE_SEPARATOR, String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test function: the separator keeps the first value it is given, so a
    /// second test setting a different one would be ignored.
    #[test]
    fn the_separator_defaults_to_a_dot_until_it_is_set() {
        assert_eq!(namespace_separator(), ".");

        set_namespace_separator("_".to_owned());
        assert_eq!(namespace_separator(), "_");

        set_namespace_separator("::".to_owned());
        assert_eq!(namespace_separator(), "_", "the first value wins");
    }
}
