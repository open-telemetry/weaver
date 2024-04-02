// SPDX-License-Identifier: Apache-2.0

//! A Rust library for loading and validating telemetry schemas.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use serde::{Deserialize, Serialize};
use url::Url;
use weaver_semconv::path::RegistryPath;

use weaver_semconv::SemConvRegistry;
use weaver_version::Versions;

use crate::event::Event;
use crate::metric_group::MetricGroup;
use crate::schema_spec::SchemaSpec;
use crate::span::Span;

pub mod attribute;
pub mod event;
pub mod instrumentation_library;
pub mod log;
pub mod metric_group;
pub mod resource;
pub mod resource_events;
pub mod resource_metrics;
pub mod resource_spans;
pub mod schema_spec;
pub mod span;
pub mod span_event;
pub mod span_link;
pub mod tags;
pub mod univariate_metric;

/// An error that can occur while loading a telemetry schema.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// The telemetry schema was not found.
    #[error("Schema {path_or_url:?} not found\n{error:?}")]
    SchemaNotFound {
        /// The path or URL of the telemetry schema.
        path_or_url: String,
        /// The error that occurred.
        error: String,
    },

    /// The telemetry schema is invalid.
    #[error("Invalid schema {path_or_url:?}\n{error:?}")]
    InvalidSchema {
        /// The path or URL of the telemetry schema.
        path_or_url: String,
        /// The line number where the error occurred.
        line: Option<usize>,
        /// The column number where the error occurred.
        column: Option<usize>,
        /// The error that occurred.
        error: String,
    },

    /// The attribute is invalid.
    #[error("Invalid attribute `{id:?}`\n{error:?}")]
    InvalidAttribute {
        /// The attribute id.
        id: String,
        /// The error that occurred.
        error: String,
    },
}

/// A telemetry schema.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct TelemetrySchema {
    /// Defines the file format. MUST be set to 1.2.0.
    pub file_format: String,
    /// Optional field specifying the schema url of the parent schema. The current
    /// schema overrides the parent schema.
    /// Usually the parent schema is the official OpenTelemetry Telemetry schema
    /// containing the versioning and their corresponding transformations.
    /// However, it can also include any of the new fields defined in this OTEP.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_schema_url: Option<String>,
    /// The Schema URL that this file is published at.
    pub schema_url: String,
    /// The semantic conventions that are imported by the current schema (optional).
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub semantic_conventions: Vec<RegistryPath>,
    /// Definition of the telemetry schema for an application or a library.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<SchemaSpec>,
    /// Definitions for each schema version in this family.
    /// Note: the ordering of versions is defined according to semver
    /// version number ordering rules.
    /// This section is described in more details in the OTEP 0152 and in a dedicated
    /// section below.
    /// <https://github.com/open-telemetry/oteps/blob/main/text/0152-telemetry-schemas.md>
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versions: Option<Versions>,

    /// The parent schema.
    #[serde(skip)]
    pub parent_schema: Option<Box<TelemetrySchema>>,

    /// The semantic convention registry used to resolve the schema
    /// (if resolved).
    #[serde(skip)]
    pub semantic_convention_registry: SemConvRegistry,
}

impl TelemetrySchema {
    /// Loads a telemetry schema from an URL or a local path.
    pub fn load(schema: &str) -> Result<TelemetrySchema, Error> {
        if schema.starts_with("http://") || schema.starts_with("https://") {
            let schema_url = Url::parse(schema).map_err(|e| Error::SchemaNotFound {
                path_or_url: schema.to_owned(),
                error: e.to_string(),
            })?;
            Self::load_from_url(&schema_url)
        } else {
            Self::load_from_file(schema)
        }
    }

    /// Loads a telemetry schema file and returns the schema.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<TelemetrySchema, Error> {
        let path_buf = path.as_ref().to_path_buf();

        // Load and deserialize the telemetry schema
        let schema_file = File::open(path).map_err(|e| Error::SchemaNotFound {
            path_or_url: path_buf.as_path().display().to_string(),
            error: e.to_string(),
        })?;
        let schema: TelemetrySchema = serde_yaml::from_reader(BufReader::new(schema_file))
            .map_err(|e| Error::InvalidSchema {
                path_or_url: path_buf.as_path().display().to_string(),
                line: e.location().map(|loc| loc.line()),
                column: e.location().map(|loc| loc.column()),
                error: e.to_string(),
            })?;

        Ok(schema)
    }

    /// Loads a telemetry schema from a URL and returns the schema.
    pub fn load_from_url(schema_url: &Url) -> Result<TelemetrySchema, Error> {
        match schema_url.scheme() {
            "http" | "https" => {
                // Create a content reader from the schema URL
                let reader = ureq::get(schema_url.as_ref())
                    .call()
                    .map_err(|e| Error::SchemaNotFound {
                        path_or_url: schema_url.to_string(),
                        error: e.to_string(),
                    })?
                    .into_reader();

                // Deserialize the telemetry schema from the content reader
                let schema: TelemetrySchema =
                    serde_yaml::from_reader(reader).map_err(|e| Error::InvalidSchema {
                        path_or_url: schema_url.to_string(),
                        line: e.location().map(|loc| loc.line()),
                        column: e.location().map(|loc| loc.column()),
                        error: e.to_string(),
                    })?;
                Ok(schema)
            }
            "file" => {
                let path = schema_url.path();
                Self::load_from_file(path)
            }
            _ => Err(Error::SchemaNotFound {
                path_or_url: schema_url.to_string(),
                error: format!("Unsupported URL scheme: {}", schema_url.scheme()),
            }),
        }
    }

    /// Sets the semantic convention catalog used to resolve the schema.
    pub fn set_semantic_convention_catalog(&mut self, catalog: SemConvRegistry) {
        self.semantic_convention_registry = catalog;
    }

    /// Sets the parent schema.
    pub fn set_parent_schema(&mut self, parent_schema: Option<TelemetrySchema>) {
        self.parent_schema = parent_schema.map(Box::new);
    }

    /// Returns the semantic conventions for the schema and its parent schemas.
    #[must_use]
    pub fn merged_semantic_conventions(&self) -> Vec<RegistryPath> {
        let mut result = vec![];
        if let Some(parent_schema) = self.parent_schema.as_ref() {
            result.extend(parent_schema.merged_semantic_conventions().iter().cloned());
        }
        result.extend(self.semantic_conventions.iter().cloned());
        result
    }

    /// Merges versions from the parent schema into the current schema.
    pub fn merge_versions(&mut self) {
        if let Some(parent_schema) = &self.parent_schema {
            match self.versions {
                Some(ref mut versions) => {
                    if let Some(parent_versions) = parent_schema.versions.as_ref() {
                        versions.extend(parent_versions.clone());
                    }
                }
                None => {
                    self.versions.clone_from(&parent_schema.versions);
                }
            }
        }
    }

    /// Returns the semantic convention catalog used to resolve the schema (if resolved).
    pub fn semantic_convention_catalog(&self) -> &SemConvRegistry {
        &self.semantic_convention_registry
    }

    /// Returns the number of metrics.
    #[must_use]
    pub fn metrics_count(&self) -> usize {
        self.schema
            .as_ref()
            .map_or(0, |schema| schema.metrics_count())
    }

    /// Returns the number of metric groups.
    #[must_use]
    pub fn metric_groups_count(&self) -> usize {
        self.schema
            .as_ref()
            .map_or(0, |schema| schema.metric_groups_count())
    }

    /// Returns the number of events.
    #[must_use]
    pub fn events_count(&self) -> usize {
        self.schema
            .as_ref()
            .map_or(0, |schema| schema.events_count())
    }

    /// Returns the number of spans.
    #[must_use]
    pub fn spans_count(&self) -> usize {
        self.schema
            .as_ref()
            .map_or(0, |schema| schema.spans_count())
    }

    /// Returns the number of versions.
    #[must_use]
    pub fn version_count(&self) -> usize {
        self.versions.as_ref().map_or(0, |versions| versions.len())
    }

    /// Returns the metric by name or None if not found.
    #[must_use]
    pub fn metric(&self, metric_name: &str) -> Option<&univariate_metric::UnivariateMetric> {
        self.schema
            .as_ref()
            .and_then(|schema| schema.metric(metric_name))
    }

    /// Returns the metric group by name or None if not found.
    #[must_use]
    pub fn metric_group(&self, name: &str) -> Option<&MetricGroup> {
        self.schema
            .as_ref()
            .and_then(|schema| schema.metric_group(name))
    }

    /// Returns a resource or None if not found.
    #[must_use]
    pub fn resource(&self) -> Option<&resource::Resource> {
        self.schema.as_ref().and_then(|schema| schema.resource())
    }

    /// Returns a vector of metrics.
    #[must_use]
    pub fn metrics(&self) -> Vec<&univariate_metric::UnivariateMetric> {
        self.schema.as_ref().map_or(
            Vec::<&univariate_metric::UnivariateMetric>::new(),
            |schema| schema.metrics(),
        )
    }

    /// Returns a vector of metric groups.
    #[must_use]
    pub fn metric_groups(&self) -> Vec<&MetricGroup> {
        self.schema
            .as_ref()
            .map_or(Vec::<&MetricGroup>::new(), |schema| schema.metric_groups())
    }

    /// Returns an iterator over the events.
    #[must_use]
    pub fn events(&self) -> Vec<&Event> {
        self.schema
            .as_ref()
            .map_or(Vec::<&Event>::new(), |schema| schema.events())
    }

    /// Returns a slice of spans.
    #[must_use]
    pub fn spans(&self) -> Vec<&Span> {
        self.schema
            .as_ref()
            .map_or(Vec::<&Span>::new(), |schema| schema.spans())
    }

    /// Returns an event by name or None if not found.
    #[must_use]
    pub fn event(&self, event_name: &str) -> Option<&Event> {
        self.schema
            .as_ref()
            .and_then(|schema| schema.event(event_name))
    }

    /// Returns a span by name or None if not found.
    #[must_use]
    pub fn span(&self, span_name: &str) -> Option<&Span> {
        self.schema
            .as_ref()
            .and_then(|schema| schema.span(span_name))
    }
}

#[cfg(test)]
mod test {
    use crate::TelemetrySchema;

    #[test]
    fn load_root_schema() {
        let schema = TelemetrySchema::load_from_file("data/root-schema-1.21.0.yaml");
        assert!(schema.is_ok(), "{:#?}", schema.err().unwrap());
    }

    #[test]
    fn load_app_telemetry_schema() {
        let schema = TelemetrySchema::load_from_file("../../data/app-telemetry-schema.yaml");
        assert!(schema.is_ok(), "{:#?}", schema.err().unwrap());
    }
}
