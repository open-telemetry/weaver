// SPDX-License-Identifier: Apache-2.0

//! HTTP server for testing purposes.

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::task::AbortHandle;
use tower_http::services::ServeDir;

/// Shared tokio runtime for all test HTTP servers.
static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("failed to create test runtime"));

/// An error that can occur while starting the HTTP server.
#[derive(thiserror::Error, Debug, Clone)]
#[error("Internal HTTP server error: {error}")]
pub struct HttpServerError {
    error: String,
}

/// Internal test HTTP server. Aborts its task on drop.
struct TestHttpServer {
    abort: AbortHandle,
    port: u16,
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

impl TestHttpServer {
    fn new(router: Router) -> Result<Self, HttpServerError> {
        let (port_tx, port_rx) = std::sync::mpsc::sync_channel(1);
        let handle = RUNTIME.spawn(async move {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("failed to bind");
            port_tx
                .send(
                    listener
                        .local_addr()
                        .expect("failed to get local addr")
                        .port(),
                )
                .expect("failed to send port");
            axum::serve(listener, router).await.expect("server error");
        });
        let port = port_rx.recv().map_err(|e| HttpServerError {
            error: e.to_string(),
        })?;
        Ok(Self {
            abort: handle.abort_handle(),
            port,
        })
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}/{}", self.port, path)
    }
}

/// A struct that serves static files from a directory.
pub struct ServeStaticFiles(TestHttpServer);

impl ServeStaticFiles {
    /// Creates a new HTTP server that serves static files from a directory.
    /// Note: This server is only available for testing purposes.
    pub fn from(static_path: impl Into<PathBuf>) -> Result<Self, HttpServerError> {
        let router = Router::new().fallback_service(ServeDir::new(static_path.into()));
        Ok(Self(TestHttpServer::new(router)?))
    }

    /// Returns the port of the server.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.0.port()
    }

    /// Returns the URL for a relative path on this server.
    #[must_use]
    pub fn relative_path_to_url(&self, path: &str) -> String {
        self.0.url(path)
    }
}

async fn check_auth(
    State(expected_token): State<Arc<str>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if auth != format!("Bearer {expected_token}") {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    next.run(request).await
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
        let token: Arc<str> = expected_token.into().into();
        let router = Router::new()
            .fallback_service(ServeDir::new(static_path.into()))
            .layer(middleware::from_fn_with_state(token, check_auth));
        Ok(Self(TestHttpServer::new(router)?))
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

    /// Returns the URL for a relative path on this server.
    #[must_use]
    pub fn relative_path_to_url(&self, path: &str) -> String {
        self.0.url(path)
    }
}

/// A mock GitHub REST API server.
///
/// Serves `GET /repos/{owner}/{repo}/releases/tags/{tag}` with a caller-provided
/// JSON body, and `GET /assets/{filename}` with caller-provided binary content.
/// Any other path returns 404.
///
/// Counts every request so tests can assert caching behaviour.
pub struct MockGitHubApi {
    server: TestHttpServer,
    request_count: Arc<AtomicUsize>,
}

/// Description of a single release served by [`MockGitHubApi`].
pub struct MockRelease {
    /// The repository owner.
    pub owner: String,
    /// The repository name.
    pub repo: String,
    /// The release tag.
    pub tag: String,
    /// Assets in the release: `(filename, content)` pairs. Each asset is
    /// served at `/assets/{filename}` and the release JSON's `url` points there.
    pub assets: Vec<(String, Vec<u8>)>,
}

struct MockApiState {
    releases: Vec<MockRelease>,
}

async fn mock_github_handler(
    State(state): State<Arc<MockApiState>>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    let path = request.uri().path().to_owned();
    for release in &state.releases {
        let tags_path = format!(
            "/repos/{}/{}/releases/tags/{}",
            release.owner, release.repo, release.tag
        );
        if path == tags_path {
            let host = headers
                .get("Host")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("127.0.0.1");
            let assets: Vec<serde_json::Value> = release
                .assets
                .iter()
                .map(|(name, _)| {
                    serde_json::json!({
                        "name": name,
                        "url": format!("http://{host}/assets/{name}"),
                    })
                })
                .collect();
            return (
                StatusCode::OK,
                axum::Json(serde_json::json!({ "assets": assets })),
            )
                .into_response();
        }
        for (name, content) in &release.assets {
            if path == format!("/assets/{name}") {
                return (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
                    content.clone(),
                )
                    .into_response();
            }
        }
    }
    StatusCode::NOT_FOUND.into_response()
}

async fn count_requests(
    State(counter): State<Arc<AtomicUsize>>,
    request: Request,
    next: Next,
) -> Response {
    let _ = counter.fetch_add(1, Ordering::SeqCst);
    next.run(request).await
}

impl MockGitHubApi {
    /// Start a server serving the given releases. Returns an error if the
    /// server fails to bind to a local port.
    pub fn start(releases: Vec<MockRelease>) -> Result<Self, HttpServerError> {
        let request_count = Arc::new(AtomicUsize::new(0));
        let state = Arc::new(MockApiState { releases });
        let router = Router::new()
            .fallback(mock_github_handler)
            .layer(middleware::from_fn_with_state(
                Arc::clone(&request_count),
                count_requests,
            ))
            .with_state(state);
        Ok(Self {
            server: TestHttpServer::new(router)?,
            request_count,
        })
    }

    /// Base URL of the mock API (e.g. `http://127.0.0.1:12345`).
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
            "text/x-yaml"
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
            "text/x-yaml"
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
        if let ureq::Error::StatusCode(code) = err {
            assert_eq!(code, 404);
        } else {
            panic!("Expected StatusCode error");
        }
    }
}
