// SPDX-License-Identifier: Apache-2.0

//! An ingester that reads attribute names or name=value pairs from standard input.

use std::io::{self, BufRead};

use weaver_common::Logger;

use crate::{sample_attribute::SampleAttribute, Error, Ingester, Sample};

/// An ingester that streams attribute names or name=value pairs from standard input.
/// Implements the Ingester trait to return an iterator of SampleAttribute items.
pub struct TextStdinIngester;

impl TextStdinIngester {
    /// Create a new AttributeStdInIngester
    #[must_use]
    pub fn new() -> Self {
        TextStdinIngester
    }
}

impl Default for TextStdinIngester {
    fn default() -> Self {
        Self::new()
    }
}

impl Ingester for TextStdinIngester {
    fn ingest(
        &self,
        _logger: impl Logger + Sync + Clone,
    ) -> Result<Box<dyn Iterator<Item = Sample>>, Error> {
        let stdin = io::stdin();
        let handle = stdin.lock();
        Ok(Box::new(TextStdinIterator::new(handle)))
    }
}

/// Generic iterator that can work with any BufRead source
pub struct TextStdinIterator<R: BufRead> {
    lines: io::Lines<R>,
}

impl<R: BufRead> TextStdinIterator<R> {
    /// Create a new AttributeIterator from a BufRead source
    pub fn new(reader: R) -> Self {
        Self {
            lines: reader.lines(),
        }
    }
}

impl<R: BufRead> Iterator for TextStdinIterator<R> {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.lines.next() {
                None => return None,
                Some(line_result) => {
                    if let Ok(line) = line_result {
                        return SampleAttribute::try_from(line.as_str())
                            .ok()
                            .map(Sample::Attribute);
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

    fn create_iterator(input: &str) -> TextStdinIterator<Cursor<&str>> {
        TextStdinIterator::new(Cursor::new(input))
    }

    fn get_attribute(sample: &Sample) -> Option<&SampleAttribute> {
        match sample {
            Sample::Attribute(attr) => Some(attr),
            _ => None,
        }
    }

    #[test]
    fn test_empty_input() {
        let mut iterator = create_iterator("");
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_empty_line_terminates() {
        let mut iterator = create_iterator("attribute1\n\nattribute2");
        assert_eq!(
            get_attribute(&iterator.next().unwrap()).unwrap().name,
            "attribute1"
        );
        // Empty line should terminate the iterator
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_attribute_without_value() {
        let mut iterator = create_iterator("attribute1");
        let binding = iterator.next().unwrap();
        let attribute = get_attribute(&binding).unwrap();
        assert_eq!(attribute.name, "attribute1");
        assert!(attribute.value.is_none());
        assert!(attribute.r#type.is_none());
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_attribute_with_string_value() {
        let mut iterator = create_iterator("name=value");
        let binding = iterator.next().unwrap();
        let attribute = get_attribute(&binding).unwrap();
        assert_eq!(attribute.name, "name");
        assert_eq!(
            attribute.value.as_ref().unwrap(),
            &Value::String("value".to_owned())
        );
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_attribute_with_number_value() {
        let mut iterator = create_iterator("count=42");
        let binding = iterator.next().unwrap();
        let attribute = get_attribute(&binding).unwrap();
        assert_eq!(attribute.name, "count");
        assert_eq!(attribute.value.as_ref().unwrap(), &Value::Number(42.into()));
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_multiple_attributes() {
        let mut iterator = create_iterator("attr1\nattr2=value\nattr3");

        let binding = iterator.next().unwrap();
        let attr1 = get_attribute(&binding).unwrap();
        assert_eq!(attr1.name, "attr1");
        assert!(attr1.value.is_none());

        let binding = iterator.next().unwrap();
        let attr2 = get_attribute(&binding).unwrap();
        assert_eq!(attr2.name, "attr2");
        assert_eq!(
            attr2.value.as_ref().unwrap(),
            &Value::String("value".to_owned())
        );

        let binding = iterator.next().unwrap();
        let attr3 = get_attribute(&binding).unwrap();
        assert_eq!(attr3.name, "attr3");
        assert!(attr3.value.is_none());

        assert!(iterator.next().is_none());
    }
}
