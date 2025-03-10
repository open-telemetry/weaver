use std::io::{self, BufRead};

use crate::{sample::SampleAttribute, Error, Ingester};

/// An ingester that reads attribute names from standard input.
/// Each line from stdin is treated as a separate attribute name.
pub struct AttributeStdinIngester;

impl AttributeStdinIngester {
    /// Create a new AttributeStdinIngester
    #[must_use]
    pub fn new() -> Self {
        AttributeStdinIngester
    }
}

impl Default for AttributeStdinIngester {
    fn default() -> Self {
        Self::new()
    }
}

impl Ingester<(), Vec<SampleAttribute>> for AttributeStdinIngester {
    fn ingest(&self, _: ()) -> Result<Vec<SampleAttribute>, Error> {
        let stdin = io::stdin();
        let mut attributes = Vec::new();

        let handle = stdin.lock();

        // Process each line into a SampleAttribute
        for line_result in handle.lines() {
            let line = line_result.map_err(|e| Error::IngestError {
                error: format!("Failed to read from stdin: {}", e),
            })?;

            if !line.trim().is_empty() {
                attributes.push(SampleAttribute {
                    name: line.trim().to_owned(),
                });
            }
        }

        Ok(attributes)
    }
}
