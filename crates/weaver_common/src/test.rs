// SPDX-License-Identifier: Apache-2.0

//! HTTP server for testing purposes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rouille::{match_assets, Server};
use std::sync::mpsc::Sender;

/// An error that can occur while starting the HTTP server.
#[derive(thiserror::Error, Debug, Clone)]
#[error("Internal HTTP server error: {error}")]
pub struct HttpServerError {
    error: String,
}

/// Internal test HTTP server holding the kill switch and port.
struct TestHttpServer {
    kill_switch: Sender<()>,
    port: u16,
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        let _ = self.kill_switch.send(());
    }
}

impl TestHttpServer {
    fn new(
        server: Server<impl Fn(&rouille::Request) -> rouille::Response + Send + Sync + 'static>,
    ) -> Self {
        let port = server.server_addr().port();
        let (_, kill_switch) = server.stoppable();
        Self { kill_switch, port }
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn relative_path_to_url(&self, file: &str) -> String {
        format!("http://127.0.0.1:{}/{}", self.port, file)
    }
}

/// A struct that serves static files from a directory.
pub struct ServeStaticFiles(TestHttpServer);

impl ServeStaticFiles {
    /// Creates a new HTTP server that serves static files from a directory.
    /// Note: This server is only available for testing purposes.
    pub fn from(static_path: impl Into<PathBuf>) -> Result<Self, HttpServerError> {
        let static_path = static_path.into();
        let server = Server::new("127.0.0.1:0", move |request| {
            match_assets(request, &static_path)
        })
        .map_err(|e| HttpServerError {
            error: e.to_string(),
        })?;
        Ok(Self(TestHttpServer::new(server)))
    }

    /// Returns the port of the server.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.0.port()
    }

    /// Returns the URL of a file.
    /// The file path should be relative to the static path.
    #[must_use]
    pub fn relative_path_to_url(&self, file: &str) -> String {
        self.0.relative_path_to_url(file)
    }
}

/// An HTTP server that requires Bearer token authentication to serve static files.
/// Returns 401 Unauthorized if the `Authorization: Bearer <token>` header is missing or wrong.
pub struct ServeStaticFilesWithAuth(TestHttpServer);

impl ServeStaticFilesWithAuth {
    /// Creates a new auth-checking HTTP server.
    /// Only requests with `Authorization: Bearer <expected_token>` will receive files.
    pub fn from(
        static_path: impl Into<PathBuf>,
        expected_token: impl Into<String>,
    ) -> Result<Self, HttpServerError> {
        let static_path = static_path.into();
        let expected_token = expected_token.into();
        let server = Server::new("127.0.0.1:0", move |request| {
            let auth = request.header("Authorization").unwrap_or_default();
            let expected = format!("Bearer {expected_token}");
            if auth != expected {
                return rouille::Response::text("Unauthorized").with_status_code(401);
            }
            match_assets(request, &static_path)
        })
        .map_err(|e| HttpServerError {
            error: e.to_string(),
        })?;
        Ok(Self(TestHttpServer::new(server)))
    }

    /// Returns the port of the server.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.0.port()
    }

    /// Base URL of the server (e.g. `http://127.0.0.1:12345/`).
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.0.port())
    }

    /// Returns the URL of a file.
    #[must_use]
    pub fn relative_path_to_url(&self, file: &str) -> String {
        self.0.relative_path_to_url(file)
    }
}

/// A mock GitHub REST API server.
///
/// Serves `GET /repos/{owner}/{repo}/releases/tags/{tag}` with a caller-provided
/// JSON body, and `GET /<asset-path>` with caller-provided binary content. Any
/// other path returns 404.
///
/// Counts the number of requests it received so tests can assert caching behavior.
pub struct MockGitHubApi {
    server: TestHttpServer,
    request_count: Arc<AtomicUsize>,
}
/// Description of a single release served by [`MockGitHubApi`].
pub struct MockRelease {
    /// `{owner}/{repo}/{tag}` path components.
    pub owner: String,
    /// The repository name.
    pub repo: String,
    /// The release tag.
    pub tag: String,
    /// The assets in the release: `(filename, content)` pairs. Each asset is
    /// served at `/assets/{filename}` and the release JSON's `url` points to
    /// that same path on this server.
    pub assets: Vec<(String, Vec<u8>)>,
}

impl MockGitHubApi {
    /// Start a server serving the given releases. Returns an error if the
    /// server fails to bind to a local port.
    pub fn start(releases: Vec<MockRelease>) -> Result<Self, HttpServerError> {
        let request_count = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&request_count);
        let server = Server::new("127.0.0.1:0", move |request| {
            _ = counter.fetch_add(1, Ordering::SeqCst);
            let url = request.url();
            for release in &releases {
                let tags_path = format!(
                    "/repos/{}/{}/releases/tags/{}",
                    release.owner, release.repo, release.tag
                );
                if url == tags_path {
                    // Build a release JSON where each asset's `url` points at
                    // `/assets/{filename}` on this same server.
                    let host = request.header("Host").unwrap_or("127.0.0.1");
                    let assets_json: Vec<serde_json::Value> = release
                        .assets
                        .iter()
                        .map(|(name, _)| {
                            serde_json::json!({
                                "name": name,
                                "url": format!("http://{host}/assets/{name}"),
                            })
                        })
                        .collect();
                    let body = serde_json::json!({ "assets": assets_json });
                    return rouille::Response::from_data("application/json", body.to_string());
                }
                for (name, content) in &release.assets {
                    if url == format!("/assets/{name}") {
                        return rouille::Response::from_data(
                            "application/octet-stream",
                            content.clone(),
                        );
                    }
                }
            }
            rouille::Response::empty_404()
        })
        .map_err(|e| HttpServerError {
            error: e.to_string(),
        })?;
        Ok(Self {
            server: TestHttpServer::new(server),
            request_count,
        })
    }

    /// Base URL of the mock API (e.g. `http://127.0.0.1:12345`). Pass this to
    /// `normalize_github_url_with_api_base` in tests.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.server.port())
    }

    /// Number of HTTP requests the server has handled.
    #[must_use]
    pub fn request_count(&self) -> usize {
        self.request_count.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use crate::test::ServeStaticFiles;
    use std::io::Read;

    #[test]
    fn test_http_server() {
        let server = ServeStaticFiles::from("tests/test_data").unwrap();

        assert!(server.port() > 0);

        let content = ureq::get(&server.relative_path_to_url("file_a.yaml"))
            .call()
            .unwrap();
        assert_eq!(content.status(), 200);
        assert_eq!(
            content.headers().get("Content-Type").unwrap(),
            "application/octet-stream"
        );
        let mut body = String::new();
        _ = content
            .into_body()
            .into_reader()
            .read_to_string(&mut body)
            .unwrap();
        assert_eq!(body, "file: A");

        let content = ureq::get(&server.relative_path_to_url("file_b.yaml"))
            .call()
            .unwrap();
        assert_eq!(content.status(), 200);
        assert_eq!(
            content.headers().get("Content-Type").unwrap(),
            "application/octet-stream"
        );
        let mut body = String::new();
        _ = content
            .into_body()
            .into_reader()
            .read_to_string(&mut body)
            .unwrap();
        assert_eq!(body, "file: B");

        let result = ureq::get(&server.relative_path_to_url("unknown_file.yaml")).call();
        assert!(result.is_err());
        let err = result.unwrap_err();
        // In ureq v3, check if it's a status error with code 404
        if let ureq::Error::StatusCode(code) = err {
            assert_eq!(code, 404);
        } else {
            panic!("Expected StatusCode error");
        }
    }
}
