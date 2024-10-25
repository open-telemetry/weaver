// SPDX-License-Identifier: Apache-2.0

//! HTTP server for testing purposes.

use paris::error;
use std::ffi::OsStr;
use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::{collections::HashMap, thread::JoinHandle};
use tiny_http::{Header, Response, Server, StatusCode};

/// An error that can occur while starting the HTTP server.
#[derive(thiserror::Error, Debug, Clone)]
#[error("Internal HTTP server error: {error}")]
pub struct HttpServerError {
    error: String,
}

/// A struct that serves static files from a directory.
pub struct ServeStaticFiles {
    server: Arc<Server>,
    port: u16,
    request_handler: JoinHandle<()>,
}

impl Drop for ServeStaticFiles {
    /// Stops the HTTP server.
    fn drop(&mut self) {
        // Test to see if we can force tiny_http to kill our thread, dropping the Arc
        // before we continue to try to ensure `server` is dropped, cleaning
        // open threads.
        let mut attempts = 0;
        while !self.request_handler.is_finished() && attempts < 10 {
            self.server.unblock();
            std::thread::yield_now();
            attempts += 1;
        }
    }
}

impl ServeStaticFiles {
    /// Creates a new HTTP server that serves static files from a directory.
    /// Note: This server is only available for testing purposes.
    pub fn from(static_path: impl Into<PathBuf>) -> Result<Self, HttpServerError> {
        let server = Server::http("127.0.0.1:0").map_err(|e| HttpServerError {
            error: e.to_string(),
        })?;

        let content_types: HashMap<&'static str, &'static str> = [
            ("yaml", "application/yaml"),
            ("json", "application/json"),
            ("zip", "application/zip"),
            ("gz", "application/gzip"),
        ]
        .iter()
        .cloned()
        .collect();

        let static_path = static_path.into();
        let server = Arc::new(server);
        let server_clone = server.clone();
        let port = server
            .server_addr()
            .to_ip()
            .map(|ip| ip.port())
            .unwrap_or(0);

        let request_handler = std::thread::spawn(move || {
            for request in server_clone.incoming_requests() {
                let mut file_path = static_path.clone();
                if request.url().len() > 1 {
                    for chunk in request.url().trim_start_matches('/').split('/') {
                        file_path.push(chunk);
                    }
                }

                if !file_path.exists() {
                    let status = StatusCode(404);
                    request
                        .respond(Response::empty(status))
                        .expect("Failed to respond");
                } else if let Ok(file) = File::open(&file_path) {
                    let mut response = Response::from_file(file);
                    let content_type = file_path
                        .extension()
                        .and_then(OsStr::to_str)
                        .and_then(|ext| content_types.get(ext).copied())
                        .unwrap_or("text/plain");
                    response.add_header(
                        Header::from_str(&format!("Content-Type: {}", content_type))
                            .expect("Failed to parse header"),
                    );
                    request.respond(response).expect("Failed to respond");
                } else {
                    let status = StatusCode(500);
                    request
                        .respond(Response::empty(status))
                        .expect("Failed to respond");
                }
            }
        });

        Ok(Self {
            server,
            port,
            request_handler,
        })
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
        assert_eq!(content.header("Content-Type").unwrap(), "application/yaml");
        assert_eq!(content.into_string().unwrap(), "file: A");

        let content = ureq::get(&server.relative_path_to_url("file_b.yaml"))
            .call()
            .unwrap();
        assert_eq!(content.status(), 200);
        assert_eq!(content.header("Content-Type").unwrap(), "application/yaml");
        assert_eq!(content.into_string().unwrap(), "file: B");

        let result = ureq::get(&server.relative_path_to_url("unknown_file.yaml")).call();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ureq::Error::Status(404, _)));
    }
}
