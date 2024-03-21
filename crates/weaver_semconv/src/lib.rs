// SPDX-License-Identifier: Apache-2.0

//! This crate defines the concept of a 'semantic convention catalog', which is
//! fueled by one or more semantic convention YAML files.
//!
//! The YAML language syntax used to define a semantic convention file
//! can be found [here](https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/syntax.md).

use glob::glob;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::attribute::AttributeSpec;
use crate::group::{GroupSpec, GroupType};
use crate::metric::MetricSpec;

pub mod attribute;
pub mod group;
pub mod metric;
pub mod stability;

/// An error that can occur while loading a semantic convention registry.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// The semantic convention registry path pattern is invalid.
    #[error("Invalid semantic convention registry path pattern '{path_pattern:?}'.\n{error}")]
    InvalidRegistryPathPattern {
        /// The path pattern pointing to the semantic convention registry.
        path_pattern: String,
        /// The error that occurred.
        error: String,
    },

    /// Invalid semantic convention registry asset.
    #[error("Invalid semantic convention registry asset (registry=`{path_pattern}`).\n{error}")]
    InvalidRegistryAsset {
        /// The path pattern pointing to the semantic convention registry.
        path_pattern: String,
        /// The error that occurred.
        error: String,
    },

    /// The semantic convention asset was not found.
    #[error("Semantic convention registry {path_or_url:?} not found\n{error}")]
    CatalogNotFound {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The error that occurred.
        error: String,
    },

    /// The semantic convention asset is invalid.
    #[error("Invalid semantic convention registry {path_or_url:?}\n{error}")]
    InvalidCatalog {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The line where the error occurred.
        line: Option<usize>,
        /// The column where the error occurred.
        column: Option<usize>,
        /// The error that occurred.
        error: String,
    },

    /// The semantic convention asset contains a duplicate attribute id.
    #[error("Duplicate attribute id `{id}` detected while loading {path_or_url:?}, already defined in {origin_path_or_url:?}")]
    DuplicateAttributeId {
        /// The path or URL where the attribute id was defined for the first time.
        origin_path_or_url: String,
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The duplicated attribute id.
        id: String,
    },

    /// The semantic convention asset contains a duplicate group id.
    #[error("Duplicate group id `{id}` detected while loading {path_or_url:?} and already defined in {origin}")]
    DuplicateGroupId {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The duplicated group id.
        id: String,
        /// The asset where the group id was already defined.
        origin: String,
    },

    /// The semantic convention asset contains a duplicate metric name.
    #[error("Duplicate metric name `{name}` detected while loading {path_or_url:?}")]
    DuplicateMetricName {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The duplicated metric name.
        name: String,
    },

    /// The semantic convention asset contains an invalid attribute definition.
    #[error("Invalid attribute definition detected while resolving {path_or_url:?}, group_id=`{group_id}`.\n{error}")]
    InvalidAttribute {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the attribute.
        group_id: String,
        /// The reason of the error.
        error: String,
    },

    /// The attribute reference is not found.
    #[error("Attribute reference `{r#ref}` not found.")]
    AttributeNotFound {
        /// The attribute reference.
        r#ref: String,
    },

    /// The semantic convention asset contains an invalid metric definition.
    #[error("Invalid metric definition in {path_or_url:?}.\ngroup_id=`{group_id}`.\n{error}")]
    InvalidMetric {
        /// The path or URL of the semantic convention asset.
        path_or_url: String,
        /// The group id of the metric.
        group_id: String,
        /// The reason of the error.
        error: String,
    },
}

/// A semantic convention spec with its provenance (path or URL).
#[derive(Debug, Clone)]
pub struct SemConvSpecWithProvenance {
    /// The semantic convention spec.
    pub spec: SemConvSpec,
    /// The provenance of the semantic convention spec (path or URL).
    pub provenance: String,
}

/// A group spec with its provenance (path or URL).
#[derive(Debug, Clone)]
pub struct GroupSpecWithProvenance {
    /// The group spec.
    pub spec: GroupSpec,
    /// The provenance of the group spec (path or URL).
    pub provenance: String,
}

/// An attribute definition with its provenance (path or URL).
#[derive(Debug, Clone)]
pub struct AttributeSpecWithProvenance {
    /// The attribute definition.
    pub attribute: AttributeSpec,
    /// The provenance of the attribute (path or URL).
    pub provenance: String,
}

/// A metric definition with its provenance (path or URL).
#[derive(Debug, Clone)]
pub struct MetricSpecWithProvenance {
    /// The metric definition.
    pub metric: MetricSpec,
    /// The provenance of the metric (path or URL).
    pub provenance: String,
}

/// A semantic convention registry is a collection of semantic convention
/// specifications indexed by group id.
#[derive(Default, Debug)]
#[must_use]
pub struct SemConvRegistry {
    /// The id of the semantic convention registry.
    id: String,

    /// The number of semantic convention assets added in the semantic convention registry.
    /// A asset can be a semantic convention loaded from a file or an URL.
    asset_count: usize,

    /// A collection of semantic convention specifications loaded in the semantic convention registry.
    specs: Vec<SemConvSpecWithProvenance>,

    /// Attributes indexed by their respective id independently of their
    /// semantic convention group.
    ///
    /// This collection contains all the attributes defined in the semantic convention registry.
    all_attributes: HashMap<String, AttributeSpecWithProvenance>,

    /// Metrics indexed by their respective id.
    ///
    /// This collection contains all the metrics defined in the semantic convention registry.
    all_metrics: HashMap<String, MetricSpecWithProvenance>,

    /// Collection of attribute ids index by group id and defined in a
    /// `resource` semantic convention group.
    /// Attribute ids are references to of attributes defined in the
    /// all_attributes field.
    resource_group_attributes: HashMap<String, GroupIds>,

    /// Collection of attribute ids index by group id and defined in a
    /// `attribute_group` semantic convention group.
    /// Attribute ids are references to of attributes defined in the
    /// all_attributes field.
    attr_grp_group_attributes: HashMap<String, GroupIds>,

    /// Collection of attribute ids index by group id and defined in a
    /// `span` semantic convention group.
    /// Attribute ids are references to of attributes defined in the
    /// all_attributes field.
    span_group_attributes: HashMap<String, GroupIds>,

    /// Collection of attribute ids index by group id and defined in a
    /// `event` semantic convention group.
    /// Attribute ids are references to of attributes defined in the
    /// all_attributes field.
    event_group_attributes: HashMap<String, GroupIds>,

    /// Collection of attribute ids index by group id and defined in a
    /// `metric` semantic convention group.
    /// Attribute ids are references to of attributes defined in the
    /// all_attributes field.
    metric_group_attributes: HashMap<String, GroupIds>,

    /// Collection of attribute ids index by group id and defined in a
    /// `metric_group` semantic convention group.
    /// Attribute ids are references to of attributes defined in the
    /// all_attributes field.
    metric_group_group_attributes: HashMap<String, GroupIds>,
}

/// Statistics about the semantic convention registry.
#[must_use]
pub struct Stats {
    /// Number of semconv files.
    pub file_count: usize,
    /// Number of semconv groups.
    pub group_count: usize,
    /// Breakdown of group statistics by type.
    pub group_breakdown: HashMap<GroupType, usize>,
    /// Number of attributes.
    pub attribute_count: usize,
    /// Number of metrics.
    pub metric_count: usize,
}

/// Represents a collection of ids (attribute or metric ids).
#[derive(Debug, Default)]
struct GroupIds {
    /// The semantic convention origin (path or URL) where the group id is
    /// defined. This is used to report errors.
    origin: String,
    /// The collection of ids (attribute or metric ids).
    ids: HashSet<String>,
}

/// A semantic convention specification.
///
/// See [here](https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/syntax.md)
/// the syntax of the semantic convention YAML file.
#[derive(Serialize, Deserialize, Debug, Validate, Clone)]
#[serde(deny_unknown_fields)]
pub struct SemConvSpec {
    /// A collection of semantic convention groups.
    #[validate]
    pub groups: Vec<GroupSpec>,
}

/// The configuration of the resolver.
#[derive(Debug, Default)]
pub struct ResolverConfig {
    error_when_attribute_ref_not_found: bool,
    keep_specs: bool,
}

impl ResolverConfig {
    /// Returns a config instructing the resolver to keep
    /// the semantic convention group specs after the resolution.
    #[must_use]
    pub fn with_keep_specs() -> Self {
        Self {
            keep_specs: true,
            ..Default::default()
        }
    }
}

/// A wrapper for a resolver error that is considered as a warning
/// by configuration.
#[derive(Debug)]
pub struct ResolverWarning {
    /// The error that occurred.
    pub error: Error,
}

/// Structure to keep track of the source of the attribute to resolve.
struct AttributeToResolve {
    /// The provenance of the attribute.
    /// Path or URL of the semantic convention asset.
    path_or_url: String,
    /// The group id of the attribute.
    group_id: String,
    /// The attribute reference.
    r#ref: String,
}

/// Structure to keep track of the source of the metric to resolve.
struct MetricToResolve {
    path_or_url: String,
    group_id: String,
    r#ref: String,
}

impl SemConvRegistry {
    /// Create a new semantic convention registry.
    pub fn new(id: &str) -> Self {
        SemConvRegistry {
            id: id.into(),
            ..Default::default()
        }
    }

    /// Create a new semantic convention registry.
    ///
    /// # Arguments
    ///
    /// * `registry_id` - The id of the semantic convention registry.
    /// * `path_pattern` - A glob pattern to load semantic convention registry from files.
    ///
    /// # Returns
    ///
    /// A new semantic convention registry.
    pub fn try_from_path(registry_id: &str, path_pattern: &str) -> Result<Self, Error> {
        let mut registry = SemConvRegistry::new(registry_id);
        for sc_entry in glob(path_pattern).map_err(|e| Error::InvalidRegistryPathPattern {
            path_pattern: path_pattern.to_owned(),
            error: e.to_string(),
        })? {
            let path_buf = sc_entry.map_err(|e| Error::InvalidRegistryAsset {
                path_pattern: path_pattern.to_owned(),
                error: e.to_string(),
            })?;
            registry.load_from_file(path_buf.as_path())?;
        }
        Ok(registry)
    }

    /// Returns the id of the semantic convention registry.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Load and add a semantic convention file to the semantic convention registry.
    pub fn load_from_file<P: AsRef<Path> + Clone>(&mut self, path: P) -> Result<(), Error> {
        let spec = SemConvSpec::load_from_file(path.clone())?;
        if let Err(e) = spec.validate() {
            return Err(Error::InvalidCatalog {
                path_or_url: path.as_ref().display().to_string(),
                line: None,
                column: None,
                error: e.to_string(),
            });
        }
        self.specs.push(SemConvSpecWithProvenance {
            spec,
            provenance: path.as_ref().display().to_string(),
        });
        Ok(())
    }

    /// Loads and returns the semantic convention spec from a file.
    pub fn load_sem_conv_spec_from_file(
        sem_conv_path: &Path,
    ) -> Result<(String, SemConvSpec), Error> {
        let spec = SemConvSpec::load_from_file(sem_conv_path)?;
        if let Err(e) = spec.validate() {
            return Err(Error::InvalidCatalog {
                path_or_url: sem_conv_path.display().to_string(),
                line: None,
                column: None,
                error: e.to_string(),
            });
        }
        Ok((sem_conv_path.display().to_string(), spec))
    }

    /// Downloads and returns the semantic convention spec from an URL.
    pub fn load_sem_conv_spec_from_url(sem_conv_url: &str) -> Result<(String, SemConvSpec), Error> {
        let spec = SemConvSpec::load_from_url(sem_conv_url)?;
        if let Err(e) = spec.validate() {
            return Err(Error::InvalidCatalog {
                path_or_url: sem_conv_url.to_owned(),
                line: None,
                column: None,
                error: e.to_string(),
            });
        }
        Ok((sem_conv_url.to_owned(), spec))
    }

    /// Returns the number of semantic convention assets added in the semantic convention registry.
    #[must_use]
    pub fn asset_count(&self) -> usize {
        self.asset_count
    }

    /// Append a list of semantic convention specs to the semantic convention registry.
    pub fn append_sem_conv_specs(&mut self, specs: Vec<SemConvSpecWithProvenance>) {
        self.specs.extend(specs);
    }

    /// Append a semantic convention spec to the semantic convention registry.
    pub fn append_sem_conv_spec(&mut self, spec: SemConvSpecWithProvenance) {
        self.specs.push(spec);
        self.asset_count += 1;
    }

    /// Resolves all the references present in the semantic convention registry.
    ///
    /// The `config` parameter allows to customize the resolver behavior
    /// when a reference is not found. By default, the resolver will emit an
    /// error when a reference is not found. This behavior can be changed by
    /// setting the `error_when_<...>_ref_not_found` to `false`, in which case
    /// the resolver will record the error in a warning list and continue.
    /// The warning list is returned as a list of warnings in the result.
    pub fn resolve(&mut self, config: ResolverConfig) -> Result<Vec<ResolverWarning>, Error> {
        let mut warnings = Vec::new();
        let mut attributes_to_resolve = Vec::new();
        let mut metrics_to_resolve = HashMap::new();

        // Add all the attributes with an id to the semantic convention registry.
        for SemConvSpecWithProvenance { spec, provenance } in self.specs.clone().into_iter() {
            for group in spec.groups.iter() {
                // Process attributes
                match group.r#type {
                    GroupType::AttributeGroup
                    | GroupType::Span
                    | GroupType::Resource
                    | GroupType::Metric
                    | GroupType::Event
                    | GroupType::MetricGroup => {
                        let attributes_in_group = self.process_attributes(
                            provenance.clone(),
                            group.id.clone(),
                            group.prefix.clone(),
                            group.attributes.clone(),
                            &mut attributes_to_resolve,
                        )?;

                        let group_attributes = match group.r#type {
                            GroupType::AttributeGroup => Some(&mut self.attr_grp_group_attributes),
                            GroupType::Span => Some(&mut self.span_group_attributes),
                            GroupType::Resource => Some(&mut self.resource_group_attributes),
                            GroupType::Metric => Some(&mut self.metric_group_attributes),
                            GroupType::Event => Some(&mut self.event_group_attributes),
                            GroupType::MetricGroup => Some(&mut self.metric_group_group_attributes),
                            _ => None,
                        };

                        if let Some(group_attributes) = group_attributes {
                            let prev_group_ids = group_attributes.insert(
                                group.id.clone(),
                                GroupIds {
                                    origin: provenance.clone(),
                                    ids: attributes_in_group.clone(),
                                },
                            );
                            Self::detect_duplicated_group(
                                provenance.clone(),
                                group.id.clone(),
                                prev_group_ids,
                            )?;
                        }
                    }
                    _ => {
                        panic!(
                            "Warning: group type `{:?}` not implemented yet",
                            group.r#type
                        );
                    }
                }

                // Process metrics
                match group.r#type {
                    GroupType::Metric => {
                        let metric_name = if let Some(metric_name) = group.metric_name.as_ref() {
                            metric_name.clone()
                        } else {
                            return Err(Error::InvalidMetric {
                                path_or_url: provenance.clone(),
                                group_id: group.id.clone(),
                                error: "Metric without name".to_owned(),
                            });
                        };
                        let instrument = if let Some(instrument) = group.instrument.as_ref() {
                            instrument.clone()
                        } else {
                            return Err(Error::InvalidMetric {
                                path_or_url: provenance.clone(),
                                group_id: group.id.clone(),
                                error: "Metric without instrument definition".to_owned(),
                            });
                        };

                        let prev_val = self.all_metrics.insert(
                            metric_name.clone(),
                            MetricSpecWithProvenance {
                                metric: MetricSpec {
                                    name: metric_name.clone(),
                                    brief: group.brief.clone(),
                                    note: group.note.clone(),
                                    attributes: group.attributes.clone(),
                                    instrument,
                                    unit: group.unit.clone(),
                                },
                                provenance: provenance.clone(),
                            },
                        );
                        if prev_val.is_some() {
                            return Err(Error::DuplicateMetricName {
                                path_or_url: provenance.clone(),
                                name: metric_name.clone(),
                            });
                        }

                        if let Some(r#ref) = group.extends.as_ref() {
                            let prev_val = metrics_to_resolve.insert(
                                metric_name.clone(),
                                MetricToResolve {
                                    path_or_url: provenance.clone(),
                                    group_id: group.id.clone(),
                                    r#ref: r#ref.clone(),
                                },
                            );
                            if prev_val.is_some() {
                                return Err(Error::DuplicateMetricName {
                                    path_or_url: provenance.clone(),
                                    name: r#ref.clone(),
                                });
                            }
                        }
                    }
                    GroupType::MetricGroup => {
                        panic!("Warning: group type `metric_group` not implemented yet");
                    }
                    _ => {
                        // No metrics to process
                    }
                }
            }
        }

        // Resolve all the attributes with a reference.
        for attr_to_resolve in attributes_to_resolve.into_iter() {
            let resolved_attr = self.all_attributes.get(&attr_to_resolve.r#ref);

            if resolved_attr.is_none() {
                let err = Error::InvalidAttribute {
                    path_or_url: attr_to_resolve.path_or_url.clone(),
                    group_id: attr_to_resolve.group_id.clone(),
                    error: format!("Attribute reference '{}' not found", attr_to_resolve.r#ref),
                };
                if config.error_when_attribute_ref_not_found {
                    return Err(err);
                } else {
                    warnings.push(ResolverWarning { error: err });
                }
            }
        }

        // Resolve all the metrics with an `extends` field.
        for (metric_name, metric_to_resolve) in metrics_to_resolve {
            let attribute_group = self.attr_grp_group_attributes.get(&metric_to_resolve.r#ref);
            if let Some(attr_grp) = attribute_group {
                if let Some(metric) = self.all_metrics.get_mut(&metric_name) {
                    let mut inherited_attributes = vec![];
                    for attr_id in attr_grp.ids.iter() {
                        if let Some(attr) = self.all_attributes.get(attr_id) {
                            // Note: we only keep the last attribute definition for attributes that
                            // are defined multiple times in the group.
                            inherited_attributes.push(attr.attribute.clone());
                        }
                    }
                    metric
                        .metric
                        .attributes
                        .extend(inherited_attributes.iter().cloned());
                } else {
                    return Err(Error::InvalidMetric {
                        path_or_url: metric_to_resolve.path_or_url,
                        group_id: metric_to_resolve.group_id,
                        error: format!("The metric '{}' doesn't exist", metric_name),
                    });
                }
            } else {
                warnings.push(ResolverWarning {
                    error: Error::InvalidMetric {
                        path_or_url: metric_to_resolve.path_or_url,
                        group_id: metric_to_resolve.group_id,
                        error: format!("The reference `{}` specified in the `extends` field of the '{}' metric could not be resolved", metric_to_resolve.r#ref, metric_name),
                    }
                });
            }
        }

        if !config.keep_specs {
            self.specs.clear();
        }

        Ok(warnings)
    }

    /// Returns the number of unique attributes defined in the semantic convention registry.
    #[must_use]
    pub fn attribute_count(&self) -> usize {
        self.all_attributes.len()
    }

    /// Returns the number of unique metrics defined in the semantic convention registry.
    #[must_use]
    pub fn metric_count(&self) -> usize {
        self.all_metrics.len()
    }

    /// Returns an attribute definition from its reference or `None` if the
    /// reference does not exist.
    #[must_use]
    pub fn attribute(&self, attr_ref: &str) -> Option<&AttributeSpec> {
        self.all_attributes
            .get(attr_ref)
            .map(|attr| &attr.attribute)
    }

    /// Returns an attribute definition and its provenance from its reference
    /// or `None` if the reference does not exist.
    #[must_use]
    pub fn attribute_with_provenance(
        &self,
        attr_ref: &str,
    ) -> Option<&AttributeSpecWithProvenance> {
        self.all_attributes.get(attr_ref)
    }

    /// Returns a map id -> attribute definition from an attribute group reference.
    /// Or an error if the reference does not exist.
    pub fn attributes(
        &self,
        r#ref: &str,
        r#type: GroupType,
    ) -> Result<HashMap<&String, &AttributeSpec>, Error> {
        let mut attributes = HashMap::new();
        let group_ids = match r#type {
            GroupType::AttributeGroup => self.attr_grp_group_attributes.get(r#ref),
            GroupType::Span => self.span_group_attributes.get(r#ref),
            GroupType::Event => self.event_group_attributes.get(r#ref),
            GroupType::Metric => self.metric_group_attributes.get(r#ref),
            GroupType::MetricGroup => self.metric_group_group_attributes.get(r#ref),
            GroupType::Resource => self.resource_group_attributes.get(r#ref),
            GroupType::Scope => panic!("Scope not implemented yet"),
        };
        if let Some(group_ids) = group_ids {
            for attr_id in group_ids.ids.iter() {
                if let Some(attr) = self.all_attributes.get(attr_id) {
                    // Note: we only keep the last attribute definition for attributes that
                    // are defined multiple times in the group.
                    _ = attributes.insert(attr_id, &attr.attribute);
                }
            }
        } else {
            return Err(Error::AttributeNotFound {
                r#ref: r#ref.to_owned(),
            });
        }
        Ok(attributes)
    }

    /// Returns an iterator over all the groups defined in the semantic convention registry.
    pub fn groups(&self) -> impl Iterator<Item = &GroupSpec> {
        self.specs
            .iter()
            .flat_map(|SemConvSpecWithProvenance { spec, .. }| &spec.groups)
    }

    /// Returns an iterator over all the groups defined in the semantic convention registry.
    /// Each group is associated with its provenance (path or URL).
    pub fn groups_with_provenance(&self) -> impl Iterator<Item = GroupSpecWithProvenance> + '_ {
        self.specs
            .iter()
            .flat_map(|SemConvSpecWithProvenance { spec, provenance }| {
                spec.groups.iter().map(|group| GroupSpecWithProvenance {
                    spec: group.clone(),
                    provenance: provenance.clone(),
                })
            })
    }

    /// Returns an iterator over all the attributes defined in the semantic convention registry.
    pub fn attributes_iter(&self) -> impl Iterator<Item = &AttributeSpec> {
        self.all_attributes.values().map(|attr| &attr.attribute)
    }

    /// Returns an iterator over all the metrics defined in the semantic convention registry.
    pub fn metrics_iter(&self) -> impl Iterator<Item = &MetricSpec> {
        self.all_metrics.values().map(|metric| &metric.metric)
    }

    /// Returns a metric definition from its name or `None` if the
    /// name does not exist.
    #[must_use]
    pub fn metric(&self, metric_name: &str) -> Option<&MetricSpec> {
        self.all_metrics
            .get(metric_name)
            .map(|metric| &metric.metric)
    }

    /// Returns a metric definition and its provenance from its name
    #[must_use]
    pub fn metric_with_provenance(&self, metric_name: &str) -> Option<&MetricSpecWithProvenance> {
        self.all_metrics.get(metric_name)
    }

    /// Returns an error if prev_group_ids is not `None`.
    fn detect_duplicated_group(
        path_or_url: String,
        group_id: String,
        prev_group_ids: Option<GroupIds>,
    ) -> Result<(), Error> {
        if let Some(group_ids) = prev_group_ids.as_ref() {
            return Err(Error::DuplicateGroupId {
                path_or_url,
                id: group_id,
                origin: group_ids.origin.clone(),
            });
        }
        Ok(())
    }

    /// Processes a collection of attributes passed as a parameter (`attrs`),
    /// adds attributes fully defined to the semantic convention registry, adds attributes with
    /// a reference to the list of attributes to resolve and returns a
    /// collection of attribute ids defined in the current group.
    fn process_attributes(
        &mut self,
        path_or_url: String,
        group_id: String,
        prefix: String,
        attrs: Vec<AttributeSpec>,
        attributes_to_resolve: &mut Vec<AttributeToResolve>,
    ) -> Result<HashSet<String>, Error> {
        let mut attributes_in_group = HashSet::new();
        for mut attr in attrs.into_iter() {
            match &attr {
                AttributeSpec::Id { id, .. } => {
                    // The attribute has an id, so add it to the semantic convention registry
                    // if it does not exist yet, otherwise return an error.
                    // The fully qualified attribute id is the concatenation
                    // of the prefix and the attribute id (separated by a dot).
                    let fq_attr_id = if prefix.is_empty() {
                        id.clone()
                    } else {
                        format!("{}.{}", prefix, id)
                    };
                    if let AttributeSpec::Id { id, .. } = &mut attr {
                        id.clone_from(&fq_attr_id)
                    }
                    let prev_val = self.all_attributes.insert(
                        fq_attr_id.clone(),
                        AttributeSpecWithProvenance {
                            attribute: attr,
                            provenance: path_or_url.clone(),
                        },
                    );
                    if let Some(prev_val) = prev_val {
                        return Err(Error::DuplicateAttributeId {
                            origin_path_or_url: prev_val.provenance.clone(),
                            path_or_url: path_or_url.clone(),
                            id: fq_attr_id.clone(),
                        });
                    }
                    let _ = attributes_in_group.insert(fq_attr_id.clone());
                }
                AttributeSpec::Ref { r#ref, .. } => {
                    // The attribute has a reference, so add it to the
                    // list of attributes to resolve.
                    attributes_to_resolve.push(AttributeToResolve {
                        path_or_url: path_or_url.clone(),
                        group_id: group_id.clone(),
                        r#ref: r#ref.clone(),
                    });
                    let _ = attributes_in_group.insert(r#ref.clone());
                }
            }
        }
        Ok(attributes_in_group)
    }

    /// Returns a set of stats about the semantic convention registry.
    pub fn stats(&self) -> Stats {
        Stats {
            file_count: self.specs.len(),
            group_count: self.specs.iter().map(|sc| sc.spec.groups.len()).sum(),
            group_breakdown: self
                .specs
                .iter()
                .flat_map(|sc| sc.spec.groups.iter().map(|g| g.r#type.clone()))
                .fold(HashMap::new(), |mut acc, group_type| {
                    *acc.entry(group_type).or_insert(0) += 1;
                    acc
                }),
            attribute_count: self.all_attributes.len(),
            metric_count: self.all_metrics.len(),
        }
    }
}

impl SemConvSpec {
    /// Load a semantic convention semantic convention registry from a file.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<SemConvSpec, Error> {
        let path_buf = path.as_ref().to_path_buf();

        // Load and deserialize the semantic convention semantic convention registry
        let catalog_file = File::open(path).map_err(|e| Error::CatalogNotFound {
            path_or_url: path_buf.as_path().display().to_string(),
            error: e.to_string(),
        })?;
        let catalog: SemConvSpec =
            serde_yaml::from_reader(BufReader::new(catalog_file)).map_err(|e| {
                Error::InvalidCatalog {
                    path_or_url: path_buf.as_path().display().to_string(),
                    line: e.location().map(|loc| loc.line()),
                    column: e.location().map(|loc| loc.column()),
                    error: e.to_string(),
                }
            })?;
        Ok(catalog)
    }

    /// Load a semantic convention semantic convention registry from a URL.
    pub fn load_from_url(semconv_url: &str) -> Result<SemConvSpec, Error> {
        // Create a content reader from the semantic convention URL
        let reader = ureq::get(semconv_url)
            .call()
            .map_err(|e| Error::CatalogNotFound {
                path_or_url: semconv_url.to_owned(),
                error: e.to_string(),
            })?
            .into_reader();

        // Deserialize the telemetry schema from the content reader
        let catalog: SemConvSpec =
            serde_yaml::from_reader(reader).map_err(|e| Error::InvalidCatalog {
                path_or_url: semconv_url.to_owned(),
                line: e.location().map(|loc| loc.line()),
                column: e.location().map(|loc| loc.column()),
                error: e.to_string(),
            })?;
        Ok(catalog)
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    /// Load multiple semantic convention files in the semantic convention registry.
    /// No error should be emitted.
    /// Spot check one or two pieces of loaded data.
    #[test]
    fn test_load_catalog() {
        let yaml_files = vec![
            "data/client.yaml",
            "data/cloud.yaml",
            "data/cloudevents.yaml",
            "data/database.yaml",
            "data/database-metrics.yaml",
            "data/exception.yaml",
            "data/faas.yaml",
            "data/faas-common.yaml",
            "data/faas-metrics.yaml",
            "data/http.yaml",
            "data/http-common.yaml",
            "data/http-metrics.yaml",
            "data/jvm-metrics.yaml",
            "data/media.yaml",
            "data/messaging.yaml",
            "data/network.yaml",
            "data/rpc.yaml",
            "data/rpc-metrics.yaml",
            "data/server.yaml",
            "data/source.yaml",
            "data/trace-exception.yaml",
            "data/url.yaml",
            "data/user-agent.yaml",
            "data/vm-metrics-experimental.yaml",
            "data/tls.yaml",
        ];

        let mut catalog = SemConvRegistry::default();
        for yaml in yaml_files {
            let result = catalog.load_from_file(yaml);
            assert!(result.is_ok(), "{:#?}", result.err().unwrap());
        }

        // Now let's resolve attributes and check provenance and structure is what we expect.
        let _ = catalog.resolve(ResolverConfig::with_keep_specs()).unwrap();
        assert_eq!(
            catalog
                .attribute_with_provenance("server.address")
                .unwrap()
                .provenance,
            "data/server.yaml"
        );
        let server_address = catalog.attribute("server.address").unwrap();
        assert_eq!(server_address.brief(), "Server address - domain name if available without reverse DNS lookup, otherwise IP address or Unix domain socket name.");
        assert!(!server_address.is_required());
        assert_eq!(server_address.tag(), None);
        if let AttributeSpec::Id { r#type, .. } = server_address {
            assert_eq!(format!("{}", r#type), "string");
        } else {
            panic!("Expected real AttributeSpec, not reference");
        }
        // Assert that we read things correctly and keep provenance.
        assert_eq!(
            catalog
                .metric_with_provenance("http.client.request.duration")
                .unwrap()
                .provenance,
            "data/http-metrics.yaml"
        );
    }

    /// Test the resolver with a semantic convention semantic convention registry that contains
    /// multiple references to resolve.
    /// No error or warning should be emitted.
    #[test]
    fn test_resolve_catalog() {
        let yaml_files = vec![
            "data/http-common.yaml",
            "data/http-metrics.yaml",
            "data/network.yaml",
            "data/server.yaml",
            "data/url.yaml",
            "data/exporter.yaml",
        ];

        let mut catalog = SemConvRegistry::default();
        for yaml in yaml_files {
            let result = catalog.load_from_file(yaml);
            assert!(result.is_ok(), "{:#?}", result.err().unwrap());
        }

        let result = catalog.resolve(ResolverConfig {
            error_when_attribute_ref_not_found: false,
            ..Default::default()
        });

        match result {
            Ok(warnings) => {
                if !warnings.is_empty() {
                    dbg!(&warnings);
                }
                assert!(warnings.is_empty());
            }
            Err(e) => {
                panic!("{:#?}", e);
            }
        }
    }
}
