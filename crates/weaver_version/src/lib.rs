// SPDX-License-Identifier: Apache-2.0

//! The specification of the changes to apply to the schema for different versions.

use schemars::JsonSchema;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::all_changes::AllChanges;
use crate::logs_changes::LogsChanges;
use crate::metrics_changes::MetricsChanges;
use crate::resource_changes::ResourceChanges;
use crate::spans_changes::SpansChanges;
use logs_changes::LogsChange;
use metrics_changes::MetricsChange;
use resource_changes::ResourceChange;
use serde::{Deserialize, Serialize};
use spans_changes::SpansChange;

mod all_changes;
pub mod logs_changes;
pub mod metrics_changes;
pub mod resource_changes;
pub mod schema_changes;
pub mod spans_changes;
pub mod v2;

/// An error that can occur while loading or resolving version changes.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// The `versions` file was not found.
    #[error("Versions {path_or_url:?} not found\n{error:?}")]
    VersionsNotFound {
        /// The path or URL of the `versions` file.
        path_or_url: String,
        /// The error that occurred.
        error: String,
    },

    /// The `versions` file is invalid.
    #[error("Invalid versions {path_or_url:?}\n{error:?}")]
    InvalidVersions {
        /// The path or URL of the `versions` file.
        path_or_url: String,
        /// The line number where the error occurred.
        line: Option<usize>,
        /// The column number where the error occurred.
        column: Option<usize>,
        /// The error that occurred.
        error: String,
    },
}

/// A version of the schema.
#[derive(PartialOrd, PartialEq)]
pub struct Version(semver::Version);

/// List of versions with their changes.
#[derive(Serialize, Deserialize, Debug, Default, Clone, JsonSchema)]
#[serde(transparent)]
pub struct Versions {
    versions: BTreeMap<semver::Version, VersionSpec>,
}

/// An history of changes to apply to the schema for different versions.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionSpec {
    /// The changes to apply to the following telemetry data: resource attributes,
    /// span attributes, span event attributes, log attributes, metric attributes.
    pub all: Option<AllChanges>,
    /// The changes to apply to the metrics specification for a specific version.
    pub metrics: Option<MetricsChanges>,
    /// The changes to apply to the logs specification for a specific version.
    pub logs: Option<LogsChanges>,
    /// The changes to apply to the spans specification for a specific version.
    pub spans: Option<SpansChanges>,
    /// The changes to apply to the resource specification for a specific version.
    pub resources: Option<ResourceChanges>,
}

/// The changes to apply to rename attributes and metrics for
/// a specific version.
#[derive(Default)]
pub struct VersionChanges {
    metric_old_to_new_names: HashMap<String, String>,
    metric_old_to_new_attributes: HashMap<String, String>,
    resource_old_to_new_attributes: HashMap<String, String>,
    log_old_to_new_attributes: HashMap<String, String>,
    span_old_to_new_attributes: HashMap<String, String>,
}

/// A trait to get the new name of an attribute of a resource, log or span.
pub trait VersionAttributeChanges {
    /// Returns the new name of the given attribute or the given name if the attribute
    /// has not been renamed.
    fn get_attribute_name(&self, name: &str) -> String;
}

impl Versions {
    /// Loads a `versions` file and returns an instance of `Versions` if successful
    /// or an error if the file could not be loaded or deserialized.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Versions, Error> {
        /// Versions has a transparent serde representation so we need to define a top-level
        /// struct to deserialize the `versions` file.
        #[derive(Serialize, Deserialize, Debug)]
        struct TopLevel {
            versions: Versions,
        }

        let path_buf = path.as_ref().to_path_buf();

        // Load and deserialize the telemetry schema
        let versions_file = File::open(path).map_err(|e| Error::VersionsNotFound {
            path_or_url: path_buf.as_path().display().to_string(),
            error: e.to_string(),
        })?;
        let top_level: TopLevel =
            serde_yaml::from_reader(BufReader::new(versions_file)).map_err(|e| {
                Error::InvalidVersions {
                    path_or_url: path_buf.as_path().display().to_string(),
                    line: e.location().map(|loc| loc.line()),
                    column: e.location().map(|loc| loc.column()),
                    error: e.to_string(),
                }
            })?;
        Ok(top_level.versions)
    }

    /// Returns the most recent version or None if there are no versions.
    #[must_use]
    pub fn latest_version(&self) -> Option<Version> {
        self.versions.keys().last().map(|v| Version(v.clone()))
    }

    /// Returns a vector of tuples containing the versions and their corresponding changes
    /// in ascending order.
    #[must_use]
    pub fn versions_asc(&self) -> Vec<(Version, &VersionSpec)> {
        self.versions
            .iter()
            .map(|(v, spec)| (Version(v.clone()), spec))
            .collect()
    }

    /// Returns a vector of tuples containing the versions and their corresponding changes
    /// in descending order.
    #[must_use]
    pub fn versions_desc(&self) -> Vec<(Version, &VersionSpec)> {
        self.versions
            .iter()
            .rev()
            .map(|(v, spec)| (Version(v.clone()), spec))
            .collect()
    }

    /// Returns a vector of tuples containing the versions and their corresponding changes
    /// in ascending order from the given version.
    #[must_use]
    pub fn versions_asc_from(&self, version: Version) -> Vec<(Version, &VersionSpec)> {
        self.versions
            .range(version.0..)
            .map(|(v, spec)| (Version(v.clone()), spec))
            .collect()
    }

    /// Returns a vector of tuples containing the versions and their corresponding changes
    /// in descending order from the given version.
    #[must_use]
    pub fn versions_desc_from(&self, version: &Version) -> Vec<(Version, &VersionSpec)> {
        self.versions
            .range(..=version.0.clone())
            .rev()
            .map(|(v, spec)| (Version(v.clone()), spec))
            .collect()
    }

    /// Returns the changes to apply for the given version including the changes
    /// of the previous versions.
    /// The current supported changes are:
    /// - Renaming of attributes (for resources, logs and spans)
    /// - Renaming of metrics
    #[must_use]
    pub fn version_changes_for(&self, version: &Version) -> VersionChanges {
        let mut resource_old_to_new_attributes: HashMap<String, String> = HashMap::new();
        let mut metric_old_to_new_names: HashMap<String, String> = HashMap::new();
        let mut metric_old_to_new_attributes: HashMap<String, String> = HashMap::new();
        let mut log_old_to_new_attributes: HashMap<String, String> = HashMap::new();
        let mut span_old_to_new_attributes: HashMap<String, String> = HashMap::new();

        for (_, spec) in self.versions_desc_from(version) {
            // Builds a map of old to new attribute names for the attributes that have been renamed
            // in the different versions of the resources.
            if let Some(resources) = spec.resources.as_ref() {
                resources
                    .changes
                    .iter()
                    .flat_map(|change| change.rename_attributes.attribute_map.iter())
                    .for_each(|(old_name, new_name)| {
                        if !resource_old_to_new_attributes.contains_key(old_name) {
                            _ = resource_old_to_new_attributes
                                .insert(old_name.clone(), new_name.clone());
                        }
                    });
            }

            // Builds a map of old to new metric names that have been renamed
            // in the different versions.
            if let Some(metrics) = spec.metrics.as_ref() {
                metrics
                    .changes
                    .iter()
                    .flat_map(|change| change.rename_metrics.iter())
                    .for_each(|(old_name, new_name)| {
                        if !metric_old_to_new_names.contains_key(old_name) {
                            _ = metric_old_to_new_names.insert(old_name.clone(), new_name.clone());
                        }
                    });
            }

            // Builds a map of old to new attribute names for the attributes that have been renamed
            // in the different versions of the metrics.
            if let Some(metrics) = spec.metrics.as_ref() {
                metrics
                    .changes
                    .iter()
                    .flat_map(|change| change.rename_attributes.attribute_map.iter())
                    .for_each(|(old_name, new_name)| {
                        if !metric_old_to_new_attributes.contains_key(old_name) {
                            _ = metric_old_to_new_attributes
                                .insert(old_name.clone(), new_name.clone());
                        }
                    });
            }

            // Builds a map of old to new attribute names for the attributes that have been renamed
            // in the different versions of the logs.
            if let Some(logs) = spec.logs.as_ref() {
                logs.changes
                    .iter()
                    .flat_map(|change| change.rename_attributes.attribute_map.iter())
                    .for_each(|(old_name, new_name)| {
                        if !log_old_to_new_attributes.contains_key(old_name) {
                            _ = log_old_to_new_attributes
                                .insert(old_name.clone(), new_name.clone());
                        }
                    });
            }

            // Builds a map of old to new attribute names for the attributes that have been renamed
            // in the different versions of the spans.
            if let Some(spans) = spec.spans.as_ref() {
                spans
                    .changes
                    .iter()
                    .flat_map(|change| change.rename_attributes.attribute_map.iter())
                    .for_each(|(old_name, new_name)| {
                        if !span_old_to_new_attributes.contains_key(old_name) {
                            _ = span_old_to_new_attributes
                                .insert(old_name.clone(), new_name.clone());
                        }
                    });
            }
        }

        VersionChanges {
            resource_old_to_new_attributes,
            metric_old_to_new_attributes,
            metric_old_to_new_names,
            log_old_to_new_attributes,
            span_old_to_new_attributes,
        }
    }

    /// Update the current `Versions` to include the transformations of the parent `Versions`.
    /// Transformations of the current `Versions` take precedence over the parent `Versions`.
    pub fn extend(&mut self, parent_versions: Versions) {
        for (version, spec) in parent_versions.versions {
            match self.versions.get_mut(&version) {
                Some(current_spec) => {
                    current_spec.extend(spec);
                }
                None => {
                    _ = self.versions.insert(version.clone(), spec);
                }
            }
        }
    }

    /// Returns true if the `Versions` is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.versions.is_empty()
    }

    /// Returns the number of versions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.versions.len()
    }
}

impl VersionSpec {
    /// Update the current `VersionSpec` to include the transformations of the parent `VersionSpec`.
    /// Transformations of the current `VersionSpec` take precedence over the parent `VersionSpec`.
    pub fn extend(&mut self, parent_spec: VersionSpec) {
        // Process resources
        if let Some(resources) = parent_spec.resources {
            let mut resource_change = ResourceChange::default();
            for change in resources.changes {
                'next_parent_renaming: for (old, new) in change.rename_attributes.attribute_map {
                    for local_change in self
                        .resources
                        .get_or_insert_with(ResourceChanges::default)
                        .changes
                        .iter()
                    {
                        if local_change
                            .rename_attributes
                            .attribute_map
                            .contains_key(&old)
                        {
                            // renaming already present in local changes, skip it
                            continue 'next_parent_renaming;
                        }
                    }
                    // renaming not found in local changes, add it
                    _ = resource_change
                        .rename_attributes
                        .attribute_map
                        .insert(old, new);
                }
            }
            if !resource_change.rename_attributes.attribute_map.is_empty() {
                if self
                    .resources
                    .get_or_insert_with(ResourceChanges::default)
                    .changes
                    .is_empty()
                {
                    self.resources
                        .get_or_insert_with(ResourceChanges::default)
                        .changes
                        .push(resource_change);
                } else {
                    self.resources
                        .get_or_insert_with(ResourceChanges::default)
                        .changes[0]
                        .rename_attributes
                        .attribute_map
                        .extend(resource_change.rename_attributes.attribute_map);
                }
            }
        }

        // Process metrics
        if let Some(metrics) = parent_spec.metrics {
            let mut metrics_change = MetricsChange::default();
            for change in metrics.changes {
                'next_parent_renaming: for (old, new) in change.rename_metrics {
                    for local_change in self
                        .metrics
                        .get_or_insert_with(MetricsChanges::default)
                        .changes
                        .iter()
                    {
                        if local_change.rename_metrics.contains_key(&old) {
                            // renaming already present in local changes, skip it
                            continue 'next_parent_renaming;
                        }
                    }
                    // renaming not found in local changes, add it
                    _ = metrics_change.rename_metrics.insert(old, new);
                }
            }
            if !metrics_change.rename_metrics.is_empty() {
                if self
                    .metrics
                    .get_or_insert_with(MetricsChanges::default)
                    .changes
                    .is_empty()
                {
                    self.metrics
                        .get_or_insert_with(MetricsChanges::default)
                        .changes
                        .push(metrics_change);
                } else {
                    self.metrics
                        .get_or_insert_with(MetricsChanges::default)
                        .changes[0]
                        .rename_metrics
                        .extend(metrics_change.rename_metrics);
                }
            }
        }

        // Process logs
        if let Some(logs) = parent_spec.logs {
            let mut logs_change = LogsChange::default();
            for change in logs.changes {
                'next_parent_renaming: for (old, new) in change.rename_attributes.attribute_map {
                    for local_change in self
                        .logs
                        .get_or_insert_with(LogsChanges::default)
                        .changes
                        .iter()
                    {
                        if local_change
                            .rename_attributes
                            .attribute_map
                            .contains_key(&old)
                        {
                            // renaming already present in local changes, skip it
                            continue 'next_parent_renaming;
                        }
                    }
                    // renaming not found in local changes, add it
                    _ = logs_change.rename_attributes.attribute_map.insert(old, new);
                }
            }
            if !logs_change.rename_attributes.attribute_map.is_empty() {
                if self
                    .logs
                    .get_or_insert_with(LogsChanges::default)
                    .changes
                    .is_empty()
                {
                    self.logs
                        .get_or_insert_with(LogsChanges::default)
                        .changes
                        .push(logs_change);
                } else {
                    self.logs.get_or_insert_with(LogsChanges::default).changes[0]
                        .rename_attributes
                        .attribute_map
                        .extend(logs_change.rename_attributes.attribute_map);
                }
            }
        }

        // Process spans
        if let Some(spans) = parent_spec.spans {
            let mut spans_change = SpansChange::default();
            for change in spans.changes {
                'next_parent_renaming: for (old, new) in change.rename_attributes.attribute_map {
                    for local_change in self
                        .spans
                        .get_or_insert_with(SpansChanges::default)
                        .changes
                        .iter()
                    {
                        if local_change
                            .rename_attributes
                            .attribute_map
                            .contains_key(&old)
                        {
                            // renaming already present in local changes, skip it
                            continue 'next_parent_renaming;
                        }
                    }
                    // renaming not found in local changes, add it
                    _ = spans_change
                        .rename_attributes
                        .attribute_map
                        .insert(old, new);
                }
            }
            if !spans_change.rename_attributes.attribute_map.is_empty() {
                if self
                    .spans
                    .get_or_insert_with(SpansChanges::default)
                    .changes
                    .is_empty()
                {
                    self.spans
                        .get_or_insert_with(SpansChanges::default)
                        .changes
                        .push(spans_change);
                } else {
                    self.spans.get_or_insert_with(SpansChanges::default).changes[0]
                        .rename_attributes
                        .attribute_map
                        .extend(spans_change.rename_attributes.attribute_map);
                }
            }
        }
    }
}

/// Wrapper around `VersionChanges` to get the new name of an attribute of resources.
pub struct ResourcesVersionAttributeChanges<'a> {
    version_changes: &'a VersionChanges,
}

impl VersionAttributeChanges for ResourcesVersionAttributeChanges<'_> {
    /// Returns the new name of the given resource attribute or the given name if the attribute
    /// has not been renamed.
    fn get_attribute_name(&self, name: &str) -> String {
        self.version_changes.get_resource_attribute_name(name)
    }
}

/// Wrapper around `VersionChanges` to get the new name of an attribute of metrics.
pub struct MetricsVersionAttributeChanges<'a> {
    version_changes: &'a VersionChanges,
}

impl VersionAttributeChanges for MetricsVersionAttributeChanges<'_> {
    /// Returns the new name of the given metric attribute or the given name if the attribute
    /// has not been renamed.
    fn get_attribute_name(&self, name: &str) -> String {
        self.version_changes.get_metric_attribute_name(name)
    }
}

/// Wrapper around `VersionChanges` to get the new name of an attribute of logs.
pub struct LogsVersionAttributeChanges<'a> {
    version_changes: &'a VersionChanges,
}

impl VersionAttributeChanges for LogsVersionAttributeChanges<'_> {
    /// Returns the new name of the given log attribute or the given name if the attribute
    /// has not been renamed.
    fn get_attribute_name(&self, name: &str) -> String {
        self.version_changes.get_log_attribute_name(name)
    }
}

/// Wrapper around `VersionChanges` to get the new name of an attribute of spans.
pub struct SpansVersionAttributeChanges<'a> {
    version_changes: &'a VersionChanges,
}

impl VersionAttributeChanges for SpansVersionAttributeChanges<'_> {
    /// Returns the new name of the given span attribute or the given name if the attribute
    /// has not been renamed.
    fn get_attribute_name(&self, name: &str) -> String {
        self.version_changes.get_span_attribute_name(name)
    }
}

impl VersionChanges {
    /// Returns the attribute changes to apply to the resources.
    #[must_use]
    pub fn resource_attribute_changes(&self) -> impl VersionAttributeChanges + '_ {
        ResourcesVersionAttributeChanges {
            version_changes: self,
        }
    }

    /// Returns the attribute changes to apply to the metrics.
    #[must_use]
    pub fn metric_attribute_changes(&self) -> impl VersionAttributeChanges + '_ {
        MetricsVersionAttributeChanges {
            version_changes: self,
        }
    }

    /// Returns the attribute changes to apply to the logs.
    #[must_use]
    pub fn log_attribute_changes(&self) -> impl VersionAttributeChanges + '_ {
        LogsVersionAttributeChanges {
            version_changes: self,
        }
    }

    /// Returns the attribute changes to apply to the spans.
    #[must_use]
    pub fn span_attribute_changes(&self) -> impl VersionAttributeChanges + '_ {
        SpansVersionAttributeChanges {
            version_changes: self,
        }
    }

    /// Returns the new name of the given resource attribute or the given name if the attribute
    /// has not been renamed.
    #[must_use]
    pub fn get_resource_attribute_name(&self, name: &str) -> String {
        if let Some(new_name) = self.resource_old_to_new_attributes.get(name) {
            new_name.clone()
        } else {
            name.to_owned()
        }
    }

    /// Returns the new name of the given metric attribute or the given name if the attribute
    /// has not been renamed.
    #[must_use]
    pub fn get_metric_attribute_name(&self, name: &str) -> String {
        if let Some(new_name) = self.metric_old_to_new_attributes.get(name) {
            new_name.clone()
        } else {
            name.to_owned()
        }
    }

    /// Returns the new name of the given metric or the given name if the metric
    /// has not been renamed.
    #[must_use]
    pub fn get_metric_name(&self, name: &str) -> String {
        if let Some(new_name) = self.metric_old_to_new_names.get(name) {
            new_name.clone()
        } else {
            name.to_owned()
        }
    }

    /// Returns the new name of the given log attribute or the given name if the attribute
    /// has not been renamed.
    #[must_use]
    pub fn get_log_attribute_name(&self, name: &str) -> String {
        if let Some(new_name) = self.log_old_to_new_attributes.get(name) {
            new_name.clone()
        } else {
            name.to_owned()
        }
    }

    /// Returns the new name of the given span attribute or the given name if the attribute
    /// has not been renamed.
    #[must_use]
    pub fn get_span_attribute_name(&self, name: &str) -> String {
        if let Some(new_name) = self.span_old_to_new_attributes.get(name) {
            new_name.clone()
        } else {
            name.to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Versions;

    #[test]
    fn test_ordering() {
        let versions: Versions = Versions::load_from_file("data/parent_versions.yaml").unwrap();
        let mut version = None;

        for (v, _) in versions.versions_asc() {
            if let Some(version) = version {
                assert!(v > version);
            }
            version = Some(v);
        }

        let mut version = None;

        for (v, _) in versions.versions_desc() {
            if let Some(version) = version {
                assert!(v < version);
            }
            version = Some(v);
        }
    }

    #[test]
    fn test_version_changes_for() {
        let versions: Versions = Versions::load_from_file("data/parent_versions.yaml").unwrap();
        let changes = versions.version_changes_for(versions.latest_version().as_ref().unwrap());

        // Test renaming of resource attributes
        assert_eq!(
            "user_agent.original",
            changes.get_resource_attribute_name("browser.user_agent")
        );

        // Test renaming of metric names
        assert_eq!(
            "process.runtime.jvm.cpu.recent_utilization",
            changes.get_metric_name("process.runtime.jvm.cpu.utilization")
        );

        // Test renaming of span attributes
        assert_eq!(
            "user_agent.original",
            changes.get_span_attribute_name("http.user_agent")
        );
        assert_eq!(
            "http.request.method",
            changes.get_span_attribute_name("http.method")
        );
        assert_eq!("url.full", changes.get_span_attribute_name("http.url"));
        assert_eq!(
            "net.protocol.name",
            changes.get_span_attribute_name("net.app.protocol.name")
        );
        assert_eq!(
            "cloud.resource_id",
            changes.get_span_attribute_name("faas.id")
        );
        assert_eq!(
            "db.name",
            changes.get_span_attribute_name("db.hbase.namespace")
        );
        assert_eq!(
            "db.name",
            changes.get_span_attribute_name("db.cassandra.keyspace")
        );
        assert_eq!("metric_1", changes.get_metric_name("m1"));
        assert_eq!("metric_2", changes.get_metric_name("m2"));
    }

    #[test]
    fn test_override() {
        let parent_versions = Versions::load_from_file("data/parent_versions.yaml").unwrap();
        let mut app_versions = Versions::load_from_file("data/app_versions.yaml").unwrap();

        // Update `app_version` to extend `parent_versions`
        app_versions.extend(parent_versions);

        // Transformations defined in `app_versions.yaml` overriding or
        // complementing `parent_versions.yaml`
        let v1_22 = app_versions
            .versions
            .get(&semver::Version::parse("1.22.0").unwrap())
            .unwrap();
        let observed_value = v1_22.spans.as_ref().unwrap().changes[0]
            .rename_attributes
            .attribute_map
            .get("messaging.kafka.client_id");
        assert_eq!(observed_value, Some(&"messaging.client.id".to_owned()));

        let v1_8 = app_versions
            .versions
            .get(&semver::Version::parse("1.8.0").unwrap())
            .unwrap();
        let observed_value = v1_8.spans.as_ref().unwrap().changes[0]
            .rename_attributes
            .attribute_map
            .get("db.cassandra.keyspace");
        assert_eq!(observed_value, Some(&"database.name".to_owned()));
        let observed_value = v1_8.spans.as_ref().unwrap().changes[0]
            .rename_attributes
            .attribute_map
            .get("db.cassandra.keyspace");
        assert_eq!(observed_value, Some(&"database.name".to_owned()));
        let observed_value = v1_8.spans.as_ref().unwrap().changes[0]
            .rename_attributes
            .attribute_map
            .get("db.hbase.namespace");
        assert_eq!(observed_value, Some(&"db.name".to_owned()));
        let observed_value = v1_8.logs.as_ref().unwrap().changes[0]
            .rename_attributes
            .attribute_map
            .get("db.cassandra.keyspace");
        assert_eq!(observed_value, Some(&"database.name".to_owned()));
        let observed_value = v1_8.logs.as_ref().unwrap().changes[0]
            .rename_attributes
            .attribute_map
            .get("db.hbase.namespace");
        assert_eq!(observed_value, Some(&"db.name".to_owned()));
        let observed_value = v1_8.metrics.as_ref().unwrap().changes[0]
            .rename_metrics
            .get("m1");
        assert_eq!(observed_value, Some(&"metric_1".to_owned()));
        let observed_value = v1_8.metrics.as_ref().unwrap().changes[0]
            .rename_metrics
            .get("m2");
        assert_eq!(observed_value, Some(&"metric2".to_owned()));

        let v1_7_1 = app_versions
            .versions
            .get(&semver::Version::parse("1.7.1").unwrap())
            .unwrap();
        let observed_value = v1_7_1.spans.as_ref().unwrap().changes[0]
            .rename_attributes
            .attribute_map
            .get("db.cassandra.table");
        assert_eq!(observed_value, Some(&"database.table".to_owned()));

        // Transformations inherited from `parent_versions.yaml` and
        // initially not present in `app_versions.yaml`
        let v1_21 = app_versions
            .versions
            .get(&semver::Version::parse("1.21.0").unwrap())
            .unwrap();
        let observed_value = v1_21.metrics.as_ref().unwrap().changes[0]
            .rename_metrics
            .get("process.runtime.jvm.cpu.utilization");
        assert_eq!(
            observed_value,
            Some(&"process.runtime.jvm.cpu.recent_utilization".to_owned())
        );
        let observed_value = v1_21.spans.as_ref().unwrap().changes[0]
            .rename_attributes
            .attribute_map
            .get("messaging.kafka.client_id");
        assert_eq!(observed_value, Some(&"messaging.client_id".to_owned()));

        let changes =
            app_versions.version_changes_for(app_versions.latest_version().as_ref().unwrap());
        assert_eq!("metric_1", changes.get_metric_name("m1"));
        assert_eq!("metric2", changes.get_metric_name("m2"));
    }
}
