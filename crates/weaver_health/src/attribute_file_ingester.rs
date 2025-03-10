use std::fs;
use std::path::Path;

use crate::{sample::SampleAttribute, Error, Ingester};

/// An ingester that reads attribute names from a text file.
/// Each line in the file is treated as a separate attribute name.
pub struct AttributeFileIngester;

impl AttributeFileIngester {
    /// Create a new AttributeFileIngester
    #[must_use]
    pub fn new() -> Self {
        AttributeFileIngester
    }
}

impl Default for AttributeFileIngester {
    fn default() -> Self {
        Self::new()
    }
}

impl Ingester<&Path, Vec<SampleAttribute>> for AttributeFileIngester {
    fn ingest(&self, input: &Path) -> Result<Vec<SampleAttribute>, Error> {
        // Read the file contents
        let content = fs::read_to_string(input).map_err(|e| Error::IngestError {
            error: format!("Failed to read file {}: {}", input.display(), e),
        })?;

        // Process each line into a SampleAttribute
        let attributes = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| SampleAttribute {
                name: line.trim().to_owned(),
            })
            .collect();

        Ok(attributes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

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
        let ingester = AttributeFileIngester::new();
        let result = ingester.ingest(&file_path).unwrap();

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
        let ingester = AttributeFileIngester::new();
        let result = ingester.ingest(&file_path).unwrap();

        // Verify the results
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_file_not_found() {
        let non_existent_path = Path::new("/path/to/nonexistent/file.txt");
        let ingester = AttributeFileIngester::new();
        let result = ingester.ingest(non_existent_path);

        assert!(result.is_err());
        if let Err(Error::IngestError { error }) = result {
            assert!(error.contains("Failed to read file"));
        } else {
            panic!("Expected IngestError");
        }
    }
}
