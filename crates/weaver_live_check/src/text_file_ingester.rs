// SPDX-License-Identifier: Apache-2.0

//! An ingester that reads attribute names or name=value pairs from a text file.

use std::path::Path;
use std::{fs, path::PathBuf};

use weaver_common::Logger;

use crate::Sample;
use crate::{sample_attribute::SampleAttribute, Error, Ingester};

/// An ingester that reads attributes from a text file.
/// Each line in the file is treated as a separate attribute.
pub struct TextFileIngester {
    path: PathBuf,
}

impl TextFileIngester {
    /// Create a new AttributeFileIngester
    #[must_use]
    pub fn new(path: &Path) -> Self {
        TextFileIngester {
            path: path.to_path_buf(),
        }
    }
}

impl Ingester for TextFileIngester {
    fn ingest(
        &self,
        _logger: impl Logger + Sync + Clone,
    ) -> Result<Box<dyn Iterator<Item = Sample>>, Error> {
        // Read the file contents
        let content = fs::read_to_string(&self.path).map_err(|e| Error::IngestError {
            error: format!("Failed to read file {}: {}", self.path.display(), e),
        })?;

        let mut attributes = Vec::new();
        // Process each line into a SampleAttribute
        for line in content.lines() {
            if let Ok(sample_attribute) = SampleAttribute::try_from(line) {
                attributes.push(Sample::Attribute(sample_attribute));
            }
        }

        Ok(Box::new(attributes.into_iter()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use weaver_common::TestLogger;

    fn get_attribute(sample: &Sample) -> Option<&SampleAttribute> {
        match sample {
            Sample::Attribute(attr) => Some(attr),
            _ => None,
        }
    }

    #[test]
    fn test_attribute_file_ingestion() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_attributes.txt");

        // Write test data to the file
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "aws.s3.bucket").unwrap();
        writeln!(file, "aws.s3.bucket.name").unwrap();
        writeln!(file, "task.id").unwrap();
        writeln!(file, "TaskId").unwrap();

        // Create ingester and process the file
        let ingester = TextFileIngester::new(&file_path);
        let logger = TestLogger::new();
        let result = ingester.ingest(logger).unwrap().collect::<Vec<_>>();

        // Verify the results
        assert_eq!(result.len(), 4);
        assert_eq!(get_attribute(&result[0]).unwrap().name, "aws.s3.bucket");
        assert_eq!(
            get_attribute(&result[1]).unwrap().name,
            "aws.s3.bucket.name"
        );
        assert_eq!(get_attribute(&result[2]).unwrap().name, "task.id");
        assert_eq!(get_attribute(&result[3]).unwrap().name, "TaskId");
    }

    #[test]
    fn test_empty_file() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("empty.txt");

        // Create an empty file
        let _ = File::create(&file_path).unwrap();

        // Create ingester and process the file
        let ingester = TextFileIngester::new(&file_path);

        let logger = TestLogger::new();
        let result = ingester.ingest(logger).unwrap().collect::<Vec<_>>();

        // Verify the results
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_file_not_found() {
        let non_existent_path = Path::new("/path/to/nonexistent/file.txt");
        let ingester = TextFileIngester::new(non_existent_path);

        let logger = TestLogger::new();
        let result = ingester.ingest(logger);

        assert!(result.is_err());
        if let Err(Error::IngestError { error }) = result {
            assert!(error.contains("Failed to read file"));
        } else {
            panic!("Expected IngestError");
        }
    }

    #[test]
    fn test_name_value_parsing() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("name_value.txt");

        // Write test data to the file with name=value format
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "simple=value").unwrap();
        writeln!(file, "number=123").unwrap();
        writeln!(file, "boolean=true").unwrap();
        writeln!(file, "string_array=[\"one\", \"two\", \"three\"]").unwrap();
        writeln!(file, "equals_sign=test=test").unwrap();

        // Create ingester and process the file
        let ingester = TextFileIngester::new(&file_path);
        let logger = TestLogger::new();
        let result = ingester.ingest(logger).unwrap().collect::<Vec<_>>();

        // Verify the results
        assert_eq!(result.len(), 5);

        assert_eq!(get_attribute(&result[0]).unwrap().name, "simple");
        assert_eq!(
            get_attribute(&result[0])
                .unwrap()
                .value
                .as_ref()
                .unwrap()
                .as_str()
                .unwrap(),
            "value"
        );

        assert_eq!(get_attribute(&result[1]).unwrap().name, "number");
        assert_eq!(
            get_attribute(&result[1])
                .unwrap()
                .value
                .as_ref()
                .unwrap()
                .as_i64()
                .unwrap(),
            123
        );

        assert_eq!(get_attribute(&result[2]).unwrap().name, "boolean");
        assert!(get_attribute(&result[2])
            .unwrap()
            .value
            .as_ref()
            .unwrap()
            .as_bool()
            .unwrap());

        assert_eq!(get_attribute(&result[3]).unwrap().name, "string_array");
        let array = get_attribute(&result[3])
            .unwrap()
            .value
            .as_ref()
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(array.len(), 3);
        assert_eq!(array[0].as_str().unwrap(), "one");
        assert_eq!(array[1].as_str().unwrap(), "two");
        assert_eq!(array[2].as_str().unwrap(), "three");

        assert_eq!(get_attribute(&result[4]).unwrap().name, "equals_sign");
        assert_eq!(
            get_attribute(&result[4])
                .unwrap()
                .value
                .as_ref()
                .unwrap()
                .as_str()
                .unwrap(),
            "test=test"
        );
    }
}
