// SPDX-License-Identifier: Apache-2.0

//! An ingester that reads attribute names or name=value pairs from standard input.

use std::io::{self, BufRead};

use serde_json::json;
use weaver_common::Logger;

use crate::{sample::SampleAttribute, Error, Ingester};

/// An ingester that streams attribute names or name=value pairs from standard input.
/// Implements the Ingester trait to return an iterator of SampleAttribute items.
pub struct AttributeStdinIngester;

impl AttributeStdinIngester {
    /// Create a new AttributeStdInIngester
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
        let stdin = io::stdin();
        let handle = stdin.lock();
        Ok(Box::new(AttributeIterator::new(handle)))
    }
}

/// Generic iterator that can work with any BufRead source
pub struct AttributeIterator<R: BufRead> {
    lines: io::Lines<R>,
}

impl<R: BufRead> AttributeIterator<R> {
    /// Create a new AttributeIterator from a BufRead source
    pub fn new(reader: R) -> Self {
        Self {
            lines: reader.lines(),
        }
    }
}

impl<R: BufRead> Iterator for AttributeIterator<R> {
    type Item = SampleAttribute;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.lines.next() {
                None => return None,
                Some(line_result) => {
                    match line_result {
                        // TODO Perhaps exit on error?
                        Err(_) => continue, // Skip lines with errors
                        Ok(line) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                // exit on empty line
                                return None;
                            }
                            // If the line follows the pattern name=value, split it
                            if let Some((name, value)) = trimmed.split_once('=') {
                                let mut sample_attribute = SampleAttribute {
                                    name: name.trim().to_owned(),
                                    value: Some(
                                        serde_json::from_str(value.trim())
                                            .unwrap_or(json!(value.trim())),
                                    ),
                                    r#type: None,
                                };
                                sample_attribute.infer_type();
                                return Some(sample_attribute);
                            }
                            // If the line is just a name, return it
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::io::Cursor;

    fn create_iterator(input: &str) -> AttributeIterator<Cursor<&str>> {
        AttributeIterator::new(Cursor::new(input))
    }

    #[test]
    fn test_empty_input() {
        let mut iterator = create_iterator("");
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_empty_line_terminates() {
        let mut iterator = create_iterator("attribute1\n\nattribute2");
        assert_eq!(iterator.next().unwrap().name, "attribute1");
        // Empty line should terminate the iterator
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_attribute_without_value() {
        let mut iterator = create_iterator("attribute1");
        let attribute = iterator.next().unwrap();
        assert_eq!(attribute.name, "attribute1");
        assert!(attribute.value.is_none());
        assert!(attribute.r#type.is_none());
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_attribute_with_string_value() {
        let mut iterator = create_iterator("name=value");
        let attribute = iterator.next().unwrap();
        assert_eq!(attribute.name, "name");
        assert_eq!(attribute.value.unwrap(), Value::String("value".to_owned()));
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_attribute_with_number_value() {
        let mut iterator = create_iterator("count=42");
        let attribute = iterator.next().unwrap();
        assert_eq!(attribute.name, "count");
        assert_eq!(attribute.value.unwrap(), Value::Number(42.into()));
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_multiple_attributes() {
        let mut iterator = create_iterator("attr1\nattr2=value\nattr3");

        let attr1 = iterator.next().unwrap();
        assert_eq!(attr1.name, "attr1");
        assert!(attr1.value.is_none());

        let attr2 = iterator.next().unwrap();
        assert_eq!(attr2.name, "attr2");
        assert_eq!(attr2.value.unwrap(), Value::String("value".to_owned()));

        let attr3 = iterator.next().unwrap();
        assert_eq!(attr3.name, "attr3");
        assert!(attr3.value.is_none());

        assert!(iterator.next().is_none());
    }
}
