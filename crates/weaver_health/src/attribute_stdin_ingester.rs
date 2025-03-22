use std::io::{self, BufRead};

use weaver_common::Logger;

use crate::{sample::SampleAttribute, Error, Ingester};

/// An ingester that streams attribute names from standard input.
/// Implements the Ingester trait to return an iterator of SampleAttribute items.
pub struct AttributeStdinIngester;

impl AttributeStdinIngester {
    /// Create a new AttributeStreamStdInIngester
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

impl Ingester<SampleAttribute> for AttributeStdinIngester {
    fn ingest(
        &self,
        _logger: impl Logger + Sync + Clone,
    ) -> Result<Box<dyn Iterator<Item = SampleAttribute>>, Error> {
        Ok(Box::new(StdinAttributeIterator::new()))
    }
}

struct StdinAttributeIterator {
    lines: io::Lines<io::StdinLock<'static>>,
}

impl StdinAttributeIterator {
    fn new() -> Self {
        let stdin = Box::leak(Box::new(io::stdin()));
        let handle = stdin.lock();
        Self {
            lines: handle.lines(),
        }
    }
}

impl Iterator for StdinAttributeIterator {
    type Item = SampleAttribute;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.lines.next() {
                None => return None,
                Some(line_result) => {
                    match line_result {
                        Err(_) => continue, // Skip lines with errors
                        Ok(line) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            return Some(SampleAttribute {
                                name: trimmed.to_owned(),
                                value: None,
                                r#type: None,
                            });
                        }
                    }
                }
            }
        }
    }
}
