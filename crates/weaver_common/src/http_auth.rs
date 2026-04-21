// SPDX-License-Identifier: Apache-2.0

//! Per-URL HTTP Bearer-token resolution for remote registry downloads.
//!
//! Longest URL-prefix match across the rules in an [`HttpAuthResolver`] wins.
//! [`TokenSource`] is also the serde shape for `[[auth]]` entries in
//! `.weaver.toml` (wrapped by `weaver_config::AuthEntry`).

use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How a Bearer token is obtained for a match rule.
///
/// Also the wire format for the `token` / `token_env` / `token_command` fields
/// of an `[[auth]]` entry in `.weaver.toml` — the variant name is lowercased
/// and becomes the field, and exactly one must be set.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TokenSource {
    /// Literal token. Discouraged — prefer `TokenEnv` or `TokenCommand` so the
    /// token does not end up committed in `.weaver.toml`.
    Token(String),
    /// Name of an environment variable to read at fetch time.
    TokenEnv(String),
    /// Argv of a helper command; its first stdout line is the token. The
    /// standard answer for short-lived tokens (e.g. `["gh", "auth", "token"]`).
    TokenCommand(Vec<String>),
}

/// A compiled match rule: a URL prefix and how to materialize its token.
#[derive(Debug, Clone)]
pub struct AuthMatchRule {
    /// URL prefix to match (longest-prefix wins across the resolver's rules).
    pub url_prefix: String,
    /// Optional diagnostic name.
    pub name: Option<String>,
    /// How to get the token.
    pub source: TokenSource,
}

/// Helper-command token cache TTL. Short enough to respect rotation, long
/// enough to amortize the cost across a dependency fan-out.
const COMMAND_CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
struct CachedToken {
    token: String,
    fetched_at: Instant,
}

/// Resolves the Bearer token for a given request URL. Clone is cheap
/// (shared state is `Arc`-wrapped). Build via `weaver_config::build_auth_resolver`
/// from `[[auth]]` entries, or directly from [`AuthMatchRule`]s via
/// [`HttpAuthResolver::new`].
#[derive(Debug, Clone, Default)]
pub struct HttpAuthResolver {
    inner: Arc<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// Rules sorted by `url_prefix` length descending, so the first match wins.
    rules: Vec<AuthMatchRule>,
    /// Cache of helper-command outputs keyed by the argv vector.
    command_cache: Mutex<HashMap<Vec<String>, CachedToken>>,
}

impl HttpAuthResolver {
    /// Build a resolver from compiled match rules. The rules are sorted
    /// internally; caller need not pre-sort.
    #[must_use]
    pub fn new(mut rules: Vec<AuthMatchRule>) -> Self {
        rules.sort_by(|a, b| b.url_prefix.len().cmp(&a.url_prefix.len()));
        Self {
            inner: Arc::new(Inner {
                rules,
                command_cache: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Empty resolver — resolves every URL to `None`. Useful for tests and for
    /// local-only paths that do not touch the network.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Resolve a Bearer token for `url`, or `None` if no rule matches (or the
    /// matching rule's token source produces no token).
    #[must_use]
    pub fn resolve(&self, url: &str) -> Option<String> {
        let rule = self
            .inner
            .rules
            .iter()
            .find(|r| url.starts_with(&r.url_prefix))?;
        self.materialize(rule)
    }

    fn materialize(&self, rule: &AuthMatchRule) -> Option<String> {
        match &rule.source {
            TokenSource::Token(t) => Some(t.clone()),
            TokenSource::TokenEnv(name) => std::env::var(name).ok().filter(|t| !t.is_empty()),
            TokenSource::TokenCommand(argv) => self.run_command_cached(argv),
        }
    }

    fn run_command_cached(&self, argv: &[String]) -> Option<String> {
        if argv.is_empty() {
            return None;
        }
        let now = Instant::now();
        {
            let cache = self
                .inner
                .command_cache
                .lock()
                .expect("HttpAuthResolver command cache poisoned");
            if let Some(cached) = cache.get(argv) {
                if now.duration_since(cached.fetched_at) < COMMAND_CACHE_TTL {
                    return Some(cached.token.clone());
                }
            }
        }
        let token = run_token_command(argv)?;
        let mut cache = self
            .inner
            .command_cache
            .lock()
            .expect("HttpAuthResolver command cache poisoned");
        _ = cache.insert(
            argv.to_vec(),
            CachedToken {
                token: token.clone(),
                fetched_at: now,
            },
        );
        Some(token)
    }
}

fn run_token_command(argv: &[String]) -> Option<String> {
    let (program, rest) = argv.split_first()?;
    let output = Command::new(program)
        .args(rest)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    if !output.status.success() {
        log::warn!(
            "Auth token_command {argv:?} exited with status {} (stderr: {})",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?.trim();
    if first_line.is_empty() {
        None
    } else {
        Some(first_line.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_resolver_returns_none() {
        let r = HttpAuthResolver::empty();
        assert!(r.resolve("https://example.com/foo").is_none());
    }

    #[test]
    fn longest_prefix_wins() {
        let r = HttpAuthResolver::new(vec![
            AuthMatchRule {
                url_prefix: "https://github.com/".to_owned(),
                name: None,
                source: TokenSource::Token("broad".to_owned()),
            },
            AuthMatchRule {
                url_prefix: "https://github.com/acme/".to_owned(),
                name: None,
                source: TokenSource::Token("narrow".to_owned()),
            },
        ]);
        assert_eq!(
            r.resolve("https://github.com/acme/repo"),
            Some("narrow".to_owned())
        );
        assert_eq!(
            r.resolve("https://github.com/other/repo"),
            Some("broad".to_owned())
        );
        assert_eq!(r.resolve("https://gitlab.com/x"), None);
    }

    #[test]
    fn env_var_lookup() {
        // Unique name to avoid collisions with other in-process tests.
        let name = "WEAVER_TEST_AUTH_ENV_LOOKUP";
        unsafe {
            std::env::set_var(name, "from-env");
        }
        let r = HttpAuthResolver::new(vec![AuthMatchRule {
            url_prefix: "https://x/".to_owned(),
            name: None,
            source: TokenSource::TokenEnv(name.to_owned()),
        }]);
        assert_eq!(r.resolve("https://x/a"), Some("from-env".to_owned()));
        unsafe {
            std::env::remove_var(name);
        }
        assert_eq!(r.resolve("https://x/a"), None);
    }

    #[test]
    fn token_command_cached_across_resolves() {
        // Use a shell command that writes its own PID so we can detect whether
        // it ran once or twice within the TTL window.
        #[cfg(unix)]
        let argv = vec![
            "sh".to_owned(),
            "-c".to_owned(),
            "echo tok-$$".to_owned(),
        ];
        #[cfg(not(unix))]
        let argv = vec!["cmd".to_owned(), "/C".to_owned(), "echo tok-%RANDOM%".to_owned()];

        let r = HttpAuthResolver::new(vec![AuthMatchRule {
            url_prefix: "https://x/".to_owned(),
            name: None,
            source: TokenSource::TokenCommand(argv),
        }]);
        let a = r.resolve("https://x/a").expect("first resolve");
        let b = r.resolve("https://x/b").expect("second resolve");
        assert_eq!(a, b, "helper command should be cached within TTL");
        assert!(a.starts_with("tok-"));
    }

    #[test]
    fn token_command_failure_yields_none() {
        let r = HttpAuthResolver::new(vec![AuthMatchRule {
            url_prefix: "https://x/".to_owned(),
            name: None,
            source: TokenSource::TokenCommand(vec!["definitely-not-a-real-binary-xyz".to_owned()]),
        }]);
        assert!(r.resolve("https://x/a").is_none());
    }

    #[test]
    fn concurrent_resolves_across_different_rules() {
        use std::thread;
        let r = HttpAuthResolver::new(vec![
            AuthMatchRule {
                url_prefix: "https://a/".to_owned(),
                name: None,
                source: TokenSource::Token("token-a".to_owned()),
            },
            AuthMatchRule {
                url_prefix: "https://b/".to_owned(),
                name: None,
                source: TokenSource::Token("token-b".to_owned()),
            },
        ]);
        let r_a = r.clone();
        let r_b = r.clone();
        let ta = thread::spawn(move || r_a.resolve("https://a/x").unwrap());
        let tb = thread::spawn(move || r_b.resolve("https://b/x").unwrap());
        assert_eq!(ta.join().unwrap(), "token-a");
        assert_eq!(tb.join().unwrap(), "token-b");
    }
}
