// SPDX-License-Identifier: Apache-2.0

//! HTTP server for testing purposes.

use paris::error;
use std::path::PathBuf;

use rouille::{match_assets, Server};
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
        let static_path = static_path.into();
        let server = Server::new("127.0.0.1:0", move |request| {
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
    use std::io::Read;

    #[test]
    fn test_http_server() {
        let server = ServeStaticFiles::from("tests/test_data").unwrap();

        assert!(server.port() > 0);

        let mut content = ureq::get(&server.relative_path_to_url("file_a.yaml"))
            .call()
            .unwrap();
        assert_eq!(content.status(), 200);
        assert_eq!(
            content.headers().get("Content-Type").unwrap(),
            "application/octet-stream"
        );
        let mut body = String::new();
        _ = content.into_body().into_reader().read_to_string(&mut body).unwrap();
        assert_eq!(body, "file: A");

        let mut content = ureq::get(&server.relative_path_to_url("file_b.yaml"))
            .call()
            .unwrap();
        assert_eq!(content.status(), 200);
        assert_eq!(
            content.headers().get("Content-Type").unwrap(),
            "application/octet-stream"
        );
        let mut body = String::new();
        _ = content.into_body().into_reader().read_to_string(&mut body).unwrap();
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
