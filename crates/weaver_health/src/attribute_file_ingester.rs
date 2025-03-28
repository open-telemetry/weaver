use std::path::Path;
use std::{fs, path::PathBuf};

use serde_json::json;
use weaver_common::Logger;

use crate::{sample::SampleAttribute, Error, Ingester};

/// An ingester that reads attribute names from a text file.
/// Each line in the file is treated as a separate attribute name.
pub struct AttributeFileIngester {
    path: PathBuf,
}

impl AttributeFileIngester {
    /// Create a new AttributeFileIngester
    #[must_use]
    pub fn new(path: &Path) -> Self {
        AttributeFileIngester {
            path: path.to_path_buf(),
        }
    }
}

impl Ingester<SampleAttribute> for AttributeFileIngester {
    fn ingest(
        &self,
        _logger: impl Logger + Sync + Clone,
    ) -> Result<Box<dyn Iterator<Item = SampleAttribute>>, Error> {
        // Read the file contents
        let content = fs::read_to_string(&self.path).map_err(|e| Error::IngestError {
            error: format!("Failed to read file {}: {}", self.path.display(), e),
        })?;

        let mut attributes = Vec::new();
        // Process each line into a SampleAttribute
        for line in content.lines() {
            // If the line follows the pattern name=value, split it
            if let Some((name, value)) = line.split_once('=') {
                let mut sample_attribute = SampleAttribute {
                    name: name.trim().to_owned(),
                    value: Some(serde_json::from_str(value.trim()).unwrap_or(json!(value.trim()))),
                    r#type: None,
                };
                sample_attribute.infer_type();
                attributes.push(sample_attribute);
            } else {
                // If the line is just a name, push it
                attributes.push(SampleAttribute {
                    name: line.trim().to_owned(),
                    value: None,
                    r#type: None,
                });
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
        let ingester = AttributeFileIngester::new(&file_path);
        let logger = TestLogger::new();
        let result = ingester.ingest(logger).unwrap().collect::<Vec<_>>();

        // Verify the results
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].name, "aws.s3.bucket");
        assert_eq!(result[1].name, "aws.s3.bucket.name");
        assert_eq!(result[2].name, "task.id");
        assert_eq!(result[3].name, "TaskId");
    }

    #[test]
    fn test_empty_file() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("empty.txt");

        // Create an empty file
        let _ = File::create(&file_path).unwrap();

        // Create ingester and process the file
        let ingester = AttributeFileIngester::new(&file_path);

        let logger = TestLogger::new();
        let result = ingester.ingest(logger).unwrap().collect::<Vec<_>>();

        // Verify the results
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_file_not_found() {
        let non_existent_path = Path::new("/path/to/nonexistent/file.txt");
        let ingester = AttributeFileIngester::new(non_existent_path);

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
        let ingester = AttributeFileIngester::new(&file_path);
        let logger = TestLogger::new();
        let result = ingester.ingest(logger).unwrap().collect::<Vec<_>>();

        // Verify the results
        assert_eq!(result.len(), 5);

        assert_eq!(result[0].name, "simple");
        assert_eq!(result[0].value.as_ref().unwrap().as_str().unwrap(), "value");

        assert_eq!(result[1].name, "number");
        assert_eq!(result[1].value.as_ref().unwrap().as_i64().unwrap(), 123);

        assert_eq!(result[2].name, "boolean");
        assert!(result[2].value.as_ref().unwrap().as_bool().unwrap());

        assert_eq!(result[3].name, "string_array");
        let array = result[3].value.as_ref().unwrap().as_array().unwrap();
        assert_eq!(array.len(), 3);
        assert_eq!(array[0].as_str().unwrap(), "one");
        assert_eq!(array[1].as_str().unwrap(), "two");
        assert_eq!(array[2].as_str().unwrap(), "three");

        assert_eq!(result[4].name, "equals_sign");
        assert_eq!(
            result[4].value.as_ref().unwrap().as_str().unwrap(),
            "test=test"
        );
    }
}
