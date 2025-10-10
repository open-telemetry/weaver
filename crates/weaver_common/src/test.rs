// SPDX-License-Identifier: Apache-2.0

//! HTTP server for testing purposes.

use paris::error;
use std::path::PathBuf;

use rouille::{match_assets, Response, Server};
use std::sync::mpsc::Sender;

/// An error that can occur while starting the HTTP server.
#[derive(thiserror::Error, Debug, Clone)]
#[error("Internal HTTP server error: {error}")]
pub struct HttpServerError {
    error: String,
}

/// A struct that serves static files from a directory.
pub struct ServeStaticFiles {
    kill_switch: Sender<()>,
    port: u16,
}

impl Drop for ServeStaticFiles {
    /// Stops the HTTP server.
    fn drop(&mut self) {
        // If we fail to kill the server, ignore it.
        let _ = self.kill_switch.send(());
    }
}

impl ServeStaticFiles {
    /// Creates a new HTTP server that serves static files from a directory.
    /// Note: This server is only available for testing purposes.
    pub fn from(static_path: impl Into<PathBuf>) -> Result<Self, HttpServerError> {
        Self::from_impl(static_path, None)
    }

    /// Same as [`Self::from`], but requires `Authorization: Bearer <token>` on every request.
    /// Note: This server is only available for testing purposes.
    pub fn from_with_bearer(
        static_path: impl Into<PathBuf>,
        token: impl Into<String>,
    ) -> Result<Self, HttpServerError> {
        Self::from_impl(static_path, Some(token.into()))
    }

    fn from_impl(
        static_path: impl Into<PathBuf>,
        token: Option<String>,
    ) -> Result<Self, HttpServerError> {
        let static_path = static_path.into();

        let server = Server::new("127.0.0.1:0", move |request| {
            if let Some(token) = token.as_ref() {
                if !request
                    .header("Authorization")
                    .map(|h| h == format!("Bearer {}", token))
                    .unwrap_or(false)
                {
                    return Response::text("Unauthorized").with_status_code(401);
                }
            }
            match_assets(request, &static_path)
        })
        .map_err(|e| HttpServerError {
            error: e.to_string(),
        })?;

        let port = server.server_addr().port();
        let (_, kill_switch) = server.stoppable();
        Ok(Self { kill_switch, port })
    }

    /// Returns the port of the server.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the URL of a file.
    /// The file path should be relative to the static path.
    #[must_use]
    pub fn relative_path_to_url(&self, file: &str) -> String {
        format!("http://127.0.0.1:{}/{}", self.port, file)
    }
}

#[cfg(test)]
mod tests {
    use crate::test::ServeStaticFiles;

    #[test]
    fn test_http_server() {
        let server = ServeStaticFiles::from("tests/test_data").unwrap();

        assert!(server.port() > 0);

        let content = ureq::get(&server.relative_path_to_url("file_a.yaml"))
            .call()
            .unwrap();
        assert_eq!(content.status(), 200);
        assert_eq!(
            content.header("Content-Type").unwrap(),
            "application/octet-stream"
        );
        assert_eq!(content.into_string().unwrap(), "file: A");

        let content = ureq::get(&server.relative_path_to_url("file_b.yaml"))
            .call()
            .unwrap();
        assert_eq!(content.status(), 200);
        assert_eq!(
            content.header("Content-Type").unwrap(),
            "application/octet-stream"
        );
        assert_eq!(content.into_string().unwrap(), "file: B");

        let result = ureq::get(&server.relative_path_to_url("unknown_file.yaml")).call();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ureq::Error::Status(404, _)));
    }

    #[test]
    fn test_http_server_with_bearer_auth() {
        let token = "token";
        let server = ServeStaticFiles::from_with_bearer("tests/test_data", token).unwrap();

        let resp = ureq::get(&server.relative_path_to_url("file_a.yaml")).call();
        assert!(resp.is_err());
        assert!(matches!(resp.unwrap_err(), ureq::Error::Status(401, _)));

        let resp = ureq::get(&server.relative_path_to_url("file_a.yaml"))
            .set("Authorization", "wrong_token")
            .call();
        assert!(resp.is_err());
        assert!(matches!(resp.unwrap_err(), ureq::Error::Status(401, _)));

        let content = ureq::get(&server.relative_path_to_url("file_a.yaml"))
            .set("Authorization", &format!("Bearer {}", token))
            .call()
            .unwrap();
        assert_eq!(content.status(), 200);
        assert_eq!(content.into_string().unwrap(), "file: A");
    }
}
