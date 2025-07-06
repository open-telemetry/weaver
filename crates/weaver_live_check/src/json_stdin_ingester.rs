// SPDX-License-Identifier: Apache-2.0

//! An ingester that reads JSON samples input via standard input.
//! This implementation currently reads until EOF and then deserializes the JSON
//! rather than reading fragments and streaming.

use std::io::{self, Read};

use crate::{Error, Ingester, Sample};

/// An ingester that reads JSON samples input via standard input.
pub struct JsonStdinIngester;

impl JsonStdinIngester {
    /// Create a new JsonStdinIngester
    #[must_use]
    pub fn new() -> Self {
        JsonStdinIngester
    }
}

impl Default for JsonStdinIngester {
    fn default() -> Self {
        Self::new()
    }
}

impl Ingester for JsonStdinIngester {
    fn ingest(&self) -> Result<Box<dyn Iterator<Item = Sample>>, Error> {
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        let mut buffer = String::new();

        // Read all content from stdin
        let _ = handle
            .read_to_string(&mut buffer)
            .map_err(|e| Error::IngestError {
                error: format!("Failed to read from stdin: {e}"),
            })?;

        // Deserialize JSON from the buffer
        let samples: Vec<Sample> =
            serde_json::from_str(&buffer).map_err(|e| Error::IngestError {
                error: format!("Failed to parse JSON from stdin: {e}"),
            })?;

        Ok(Box::new(samples.into_iter()))
    }
}
