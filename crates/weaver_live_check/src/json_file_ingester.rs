// SPDX-License-Identifier: Apache-2.0

//! An ingester that reads JSON samples from a file.

use std::path::Path;
use std::rc::Rc;
use std::{fs, path::PathBuf};

use crate::sample_resource::SampleResource;
use crate::Sample;
use crate::{Error, Ingester};

/// Attaches each resource sample to the metrics, logs and spans that follow it.
pub(crate) fn attach_resources(samples: Vec<Sample>) -> impl Iterator<Item = Sample> {
    let mut current_resource: Option<Rc<SampleResource>> = None;
    samples.into_iter().map(move |mut sample| {
        match &mut sample {
            Sample::Resource(r) => current_resource = Some(Rc::new(r.clone())),
            Sample::Metric(m) => m.resource = current_resource.clone(),
            Sample::Log(l) => l.resource = current_resource.clone(),
            Sample::Span(s) => s.resource = current_resource.clone(),
            _ => {}
        }
        sample
    })
}

/// An ingester that reads samples from a JSON file.
pub struct JsonFileIngester {
    path: PathBuf,
}

impl JsonFileIngester {
    /// Create a new JsonFileIngester
    #[must_use]
    pub fn new(path: &Path) -> Self {
        JsonFileIngester {
            path: path.to_path_buf(),
        }
    }
}

impl Ingester for JsonFileIngester {
    fn ingest(&self) -> Result<Box<dyn Iterator<Item = Sample>>, Error> {
        // Open the file and use a reader to deserialize
        let file = fs::File::open(&self.path).map_err(|e| Error::IngestError {
            error: format!("Failed to open file {}: {}", self.path.display(), e),
        })?;

        // Deserialize directly from the reader
        let attributes: Vec<Sample> =
            serde_json::from_reader(file).map_err(|e| Error::IngestError {
                error: format!(
                    "Failed to parse JSON from file {}: {}",
                    self.path.display(),
                    e
                ),
            })?;

        Ok(Box::new(attach_resources(attributes)))
    }
}

#[cfg(test)]
mod tests {
    use crate::sample_attribute::SampleAttribute;

    use super::*;
    use serde_json::Value;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use weaver_semconv::v1::attribute::PrimitiveOrArrayTypeSpec;

    fn get_attribute(sample: &Sample) -> Option<&SampleAttribute> {
        match sample {
            Sample::Attribute(attr) => Some(attr),
            _ => None,
        }
    }

    #[test]
    fn test_resources_attach_to_the_signals_that_follow() {
        let samples: Vec<Sample> = serde_json::from_value(serde_json::json!([
            {"span": {"name": "before", "kind": "internal"}},
            {"resource": {"attributes": [{"name": "service.name", "value": "cart"}]}},
            {"span": {"name": "first", "kind": "internal"}},
            {"log": {"event_name": "second"}},
            {"resource": {"attributes": [{"name": "service.name", "value": "checkout"}]}},
            {"span": {"name": "third", "kind": "internal"}}
        ]))
        .expect("Failed to parse samples");

        let services: Vec<_> = attach_resources(samples)
            .filter(|sample| !matches!(sample, Sample::Resource(_)))
            .map(|sample| {
                sample
                    .resource()
                    .and_then(|resource| resource.attributes[0].value.clone())
            })
            .collect();

        assert_eq!(
            services,
            [
                None,
                Some(Value::from("cart")),
                Some(Value::from("cart")),
                Some(Value::from("checkout"))
            ]
        );
    }

    #[test]
    fn test_json_array_ingestion() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_attributes.json");

        // Create test JSON with array format
        let json_content = r#"[
            {"attribute": {"name": "aws.s3.bucket", "value": "my-bucket"}},
            {"attribute": {"name": "aws.s3.bucket.name", "value": "my-bucket-name", "type": "string"}},
            {"attribute": {"name": "task.id", "value": 123, "type": "int"}},
            {"attribute": {"name": "metrics.count", "value": 45.6, "type": "double"}},
            {"attribute": {"name": "is.active", "value": true, "type": "boolean"}},
            {"attribute": {"name": "tags", "value": ["tag1", "tag2"], "type": "string[]"}},
            {"attribute": {"name": "TaskId"}}
        ]"#;

        // Write test data to the file
        let mut file = File::create(&file_path).unwrap();
        file.write_all(json_content.as_bytes()).unwrap();

        // Create ingester and process the file
        let ingester = JsonFileIngester::new(&file_path);

        let result = ingester.ingest().unwrap().collect::<Vec<_>>();

        // Verify the results
        assert_eq!(result.len(), 7);
        assert_eq!(get_attribute(&result[0]).unwrap().name, "aws.s3.bucket");
        assert_eq!(
            get_attribute(&result[0]).unwrap().value.as_ref().unwrap(),
            &Value::String("my-bucket".to_owned())
        );
        assert_eq!(
            get_attribute(&result[0]).unwrap().r#type,
            Some(PrimitiveOrArrayTypeSpec::String)
        );

        assert_eq!(
            get_attribute(&result[1]).unwrap().name,
            "aws.s3.bucket.name"
        );
        assert_eq!(
            get_attribute(&result[1]).unwrap().value.as_ref().unwrap(),
            &Value::String("my-bucket-name".to_owned())
        );
        assert_eq!(
            get_attribute(&result[1]).unwrap().r#type,
            Some(PrimitiveOrArrayTypeSpec::String)
        );

        assert_eq!(get_attribute(&result[2]).unwrap().name, "task.id");
        assert_eq!(
            get_attribute(&result[2]).unwrap().value.as_ref().unwrap(),
            &Value::Number(serde_json::Number::from(123))
        );
        assert_eq!(
            get_attribute(&result[2]).unwrap().r#type,
            Some(PrimitiveOrArrayTypeSpec::Int)
        );

        assert_eq!(get_attribute(&result[6]).unwrap().name, "TaskId");
        assert_eq!(get_attribute(&result[6]).unwrap().value, None);
        assert_eq!(get_attribute(&result[6]).unwrap().r#type, None);
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
        let ingester = JsonFileIngester::new(&file_path);
        let result = ingester.ingest();

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
        let ingester = JsonFileIngester::new(non_existent_path);
        let result = ingester.ingest();

        assert!(result.is_err());
        if let Err(Error::IngestError { error }) = result {
            assert!(error.contains("Failed to open file"));
        } else {
            panic!("Expected IngestError");
        }
    }
}
