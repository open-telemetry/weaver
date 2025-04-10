// SPDX-License-Identifier: Apache-2.0

//! An ingester that reads sample attributes from JSON input via standard input.
//! This implementation currently reads until EOF and then deserializes the JSON
//! rather than reading fragments and streaming.

use std::io::{self, Read};

use weaver_common::Logger;

use crate::{sample_attribute::SampleAttribute, Error, Ingester};

/// An ingester that reads sample attributes from JSON input via standard input.
/// The JSON should contain an array of objects with at least a "name" field:
/// ```json
/// [
///   {"name": "attr.name", "value": "val", "type": "string"},
///   {"name": "attr.name2"}
///   {"name": "attr.name3", "type": "string"},
/// ]
/// ```
pub struct AttributeJsonStdinIngester;

impl AttributeJsonStdinIngester {
    /// Create a new AttributeJsonStdinIngester
    #[must_use]
    pub fn new() -> Self {
        AttributeJsonStdinIngester
    }
}

impl Default for AttributeJsonStdinIngester {
    fn default() -> Self {
        Self::new()
    }
}

impl Ingester<SampleAttribute> for AttributeJsonStdinIngester {
    fn ingest(
        &self,
        _logger: impl Logger + Sync + Clone,
    ) -> Result<Box<dyn Iterator<Item = SampleAttribute>>, Error> {
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        let mut buffer = String::new();

        // Read all content from stdin
        let _ = handle
            .read_to_string(&mut buffer)
            .map_err(|e| Error::IngestError {
                error: format!("Failed to read from stdin: {}", e),
            })?;

        // Deserialize JSON from the buffer
        let attributes: Vec<SampleAttribute> =
            serde_json::from_str(&buffer).map_err(|e| Error::IngestError {
                error: format!("Failed to parse JSON from stdin: {}", e),
            })?;

        Ok(Box::new(attributes.into_iter()))
    }
}
