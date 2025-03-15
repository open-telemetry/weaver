use std::fs;
use std::path::Path;

use crate::{sample::SampleAttribute, Error, Ingester};

/// An ingester that reads attribute names and values from a JSON file.
/// The file should contain an array of objects with at least a "name" field:
/// ```json
/// [
///   {"name": "attr.name", "value": "val", "type": "string"},
///   {"name": "attr.name2"}
/// ]
/// ```
pub struct AttributeJsonFileIngester;

impl AttributeJsonFileIngester {
    /// Create a new AttributeJsonFileIngester
    #[must_use]
    pub fn new() -> Self {
        AttributeJsonFileIngester
    }
}

impl Default for AttributeJsonFileIngester {
    fn default() -> Self {
        Self::new()
    }
}

impl Ingester<&Path, Vec<SampleAttribute>> for AttributeJsonFileIngester {
    fn ingest(&self, input: &Path) -> Result<Vec<SampleAttribute>, Error> {
        // Open the file and use a reader to deserialize
        let file = fs::File::open(input).map_err(|e| Error::IngestError {
            error: format!("Failed to open file {}: {}", input.display(), e),
        })?;

        // Deserialize directly from the reader
        let mut attributes: Vec<SampleAttribute> =
            serde_json::from_reader(file).map_err(|e| Error::IngestError {
                error: format!("Failed to parse JSON from file {}: {}", input.display(), e),
            })?;

        // Infer the type of the attributes from the value
        for attribute in &mut attributes {
            attribute.infer_type();
        }

        Ok(attributes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use weaver_semconv::attribute::PrimitiveOrArrayTypeSpec;

    #[test]
    fn test_json_array_ingestion() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_attributes.json");

        // Create test JSON with array format
        let json_content = r#"[
            {"name": "aws.s3.bucket", "value": "my-bucket"},
            {"name": "aws.s3.bucket.name", "value": "my-bucket-name", "type": "string"},
            {"name": "task.id", "value": 123, "type": "int"},
            {"name": "metrics.count", "value": 45.6, "type": "double"},
            {"name": "is.active", "value": true, "type": "boolean"},
            {"name": "tags", "value": ["tag1", "tag2"], "type": "string[]"},
            {"name": "TaskId"}
        ]"#;

        // Write test data to the file
        let mut file = File::create(&file_path).unwrap();
        file.write_all(json_content.as_bytes()).unwrap();

        // Create ingester and process the file
        let ingester = AttributeJsonFileIngester::new();
        let result = ingester.ingest(&file_path).unwrap();

        // Verify the results
        assert_eq!(result.len(), 7);
        assert_eq!(result[0].name, "aws.s3.bucket");
        assert_eq!(
            result[0].value.as_ref().unwrap(),
            &Value::String("my-bucket".to_owned())
        );
        assert_eq!(result[0].r#type, Some(PrimitiveOrArrayTypeSpec::String));

        assert_eq!(result[1].name, "aws.s3.bucket.name");
        assert_eq!(
            result[1].value.as_ref().unwrap(),
            &Value::String("my-bucket-name".to_owned())
        );
        assert_eq!(result[1].r#type, Some(PrimitiveOrArrayTypeSpec::String));

        assert_eq!(result[2].name, "task.id");
        assert_eq!(
            result[2].value.as_ref().unwrap(),
            &Value::Number(serde_json::Number::from(123))
        );
        assert_eq!(result[2].r#type, Some(PrimitiveOrArrayTypeSpec::Int));

        assert_eq!(result[6].name, "TaskId");
        assert_eq!(result[6].value, None);
        assert_eq!(result[6].r#type, None);
    }

    #[test]
    fn test_invalid_json() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("invalid.json");

        // Create invalid JSON
        let json_content = r#"{ "name": "invalid" :}"#;

        // Write test data to the file
        let mut file = File::create(&file_path).unwrap();
        file.write_all(json_content.as_bytes()).unwrap();

        // Create ingester and process the file
        let ingester = AttributeJsonFileIngester::new();
        let result = ingester.ingest(&file_path);

        assert!(result.is_err());
        if let Err(Error::IngestError { error }) = result {
            assert!(error.contains("Failed to parse JSON"));
        } else {
            panic!("Expected IngestError");
        }
    }

    #[test]
    fn test_file_not_found() {
        let non_existent_path = Path::new("/path/to/nonexistent/file.json");
        let ingester = AttributeJsonFileIngester::new();
        let result = ingester.ingest(non_existent_path);

        assert!(result.is_err());
        if let Err(Error::IngestError { error }) = result {
            assert!(error.contains("Failed to open file"));
        } else {
            panic!("Expected IngestError");
        }
    }
}
