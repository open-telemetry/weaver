// SPDX-License-Identifier: Apache-2.0

//! Per-URL HTTP authentication configuration.
//!
//! `[[auth]]` entries in `.weaver.toml` bind a URL prefix to a token source.
//! When weaver fetches a remote archive, manifest, or resolved schema, the
//! target URL is matched against every entry and the longest matching prefix
//! wins. See [`AuthEntry`] for the full schema.

use schemars::JsonSchema;
use serde::Deserialize;
use weaver_common::http_auth::{AuthMatchRule, HttpAuthResolver, TokenSource as RuntimeTokenSource};

/// A single entry in the `[[auth]]` array of `.weaver.toml`.
///
/// ```toml
/// [[auth]]
/// url_prefix = "https://github.com/acme/"
/// token_env  = "ACME_GITHUB_TOKEN"
///
/// [[auth]]
/// url_prefix    = "https://github.com/"
/// token_command = ["gh", "auth", "token"]
/// ```
///
/// Exactly one of `token`, `token_env`, or `token_command` must be specified.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(inline)]
pub struct AuthEntry {
    /// Match any URL whose string starts with this prefix. Longest prefix wins.
    pub url_prefix: String,
    /// Optional diagnostic name; not used for matching.
    #[serde(default)]
    pub name: Option<String>,
    /// How to materialize the Bearer token for this entry.
    #[serde(flatten)]
    pub token: TokenSource,
}

/// How a Bearer token is obtained for an [`AuthEntry`].
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TokenSource {
    /// Literal token. Discouraged — prefer `token_env` or `token_command` so the
    /// token does not end up committed in `.weaver.toml`.
    Token(String),
    /// Name of an environment variable to read at fetch time.
    TokenEnv(String),
    /// Argv of a helper command. Its first line of stdout is the token. This is
    /// the standard answer for short-lived tokens (e.g. `["gh", "auth", "token"]`).
    TokenCommand(Vec<String>),
}

impl AuthEntry {
    /// Compile this config entry into a runtime match rule.
    #[must_use]
    pub fn to_match_rule(&self) -> AuthMatchRule {
        AuthMatchRule {
            url_prefix: self.url_prefix.clone(),
            name: self.name.clone(),
            source: match &self.token {
                TokenSource::Token(t) => RuntimeTokenSource::Literal(t.clone()),
                TokenSource::TokenEnv(n) => RuntimeTokenSource::Env(n.clone()),
                TokenSource::TokenCommand(argv) => RuntimeTokenSource::Command(argv.clone()),
            },
        }
    }
}

/// Build a runtime [`HttpAuthResolver`] from a slice of config entries.
#[must_use]
pub fn build_resolver(entries: &[AuthEntry]) -> HttpAuthResolver {
    HttpAuthResolver::new(entries.iter().map(AuthEntry::to_match_rule).collect())
}

#[cfg(test)]
mod tests {
    use crate::WeaverConfig;

    #[test]
    fn parses_all_three_token_modes() {
        let toml = r#"
[[auth]]
url_prefix = "https://github.com/acme/"
token_env  = "ACME_GITHUB_TOKEN"

[[auth]]
url_prefix    = "https://github.com/"
token_command = ["gh", "auth", "token"]

[[auth]]
url_prefix = "https://semconv.internal.acme.com/"
name       = "acme-internal"
token      = "literal"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("parse");
        assert_eq!(config.auth.len(), 3);
        assert_eq!(config.auth[0].url_prefix, "https://github.com/acme/");
        assert!(matches!(
            &config.auth[0].token,
            super::TokenSource::TokenEnv(s) if s == "ACME_GITHUB_TOKEN"
        ));
        assert!(matches!(
            &config.auth[1].token,
            super::TokenSource::TokenCommand(argv) if argv == &vec!["gh".to_owned(), "auth".to_owned(), "token".to_owned()]
        ));
        assert!(matches!(
            &config.auth[2].token,
            super::TokenSource::Token(s) if s == "literal"
        ));
        assert_eq!(config.auth[2].name.as_deref(), Some("acme-internal"));
    }

    #[test]
    fn rejects_multiple_token_modes() {
        let toml = r#"
[[auth]]
url_prefix = "https://x/"
token      = "a"
token_env  = "B"
"#;
        let err = toml::from_str::<WeaverConfig>(toml).expect_err("should fail");
        assert!(
            err.to_string().contains("token")
                || err.to_string().contains("unknown")
                || err.to_string().contains("duplicate"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let toml = r#"
[[auth]]
url_prefix = "https://x/"
token      = "t"
bogus      = true
"#;
        let err = toml::from_str::<WeaverConfig>(toml).expect_err("should fail");
        assert!(err.to_string().contains("bogus"), "unexpected: {err}");
    }

    #[test]
    fn empty_config_has_no_auth() {
        let config: WeaverConfig = toml::from_str("").expect("parse empty");
        assert!(config.auth.is_empty());
    }

    #[test]
    fn build_resolver_resolves_by_url_prefix() {
        let toml = r#"
[[auth]]
url_prefix = "https://github.com/acme/"
token      = "narrow"

[[auth]]
url_prefix = "https://github.com/"
token      = "broad"
"#;
        let config: WeaverConfig = toml::from_str(toml).expect("parse");
        let resolver = crate::build_auth_resolver(&config.auth);
        assert_eq!(
            resolver.resolve("https://github.com/acme/repo"),
            Some("narrow".to_owned())
        );
        assert_eq!(
            resolver.resolve("https://github.com/other/repo"),
            Some("broad".to_owned())
        );
        assert_eq!(resolver.resolve("https://gitlab.com/x"), None);
    }
}
