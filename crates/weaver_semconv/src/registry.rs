// SPDX-License-Identifier: Apache-2.0

//! Semantic Convention Registry Definition.

use crate::attribute::AttributeSpec;
use crate::group::{GroupSpec, GroupType};
use crate::metric::MetricSpec;
use crate::semconv::{SemConvSpec, SemConvSpecWithProvenance};
use crate::{
    AttributeSpecWithProvenance, AttributeToResolve, Error, GroupIds, GroupSpecWithProvenance,
    MetricSpecWithProvenance, MetricToResolve, ResolverConfig, ResolverWarning, Stats,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A semantic convention registry is a collection of semantic convention
/// specifications indexed by group id.
#[derive(Default, Debug)]
#[must_use]
pub struct SemConvRegistry {
    /// The id of the semantic convention registry.
    id: String,

    /// The number of semantic convention spec added in the semantic convention registry.
    semconv_spec_count: usize,

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
    /// `scope` semantic convention group.
    /// Attribute ids are references to of attributes defined in the
    /// all_attributes field.
    scope_group_attributes: HashMap<String, GroupIds>,

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

impl SemConvRegistry {
    /// Create a new semantic convention registry.
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the semantic convention registry.
    pub fn new(id: &str) -> Self {
        SemConvRegistry {
            id: id.to_owned(),
            ..Default::default()
        }
    }

    /// Creates a semantic convention registry from the given path pattern.
    ///
    /// # Arguments
    ///
    /// * `registry_id` - The id of the semantic convention registry.
    /// * `path_pattern` - The path pattern to load the semantic convention specs.
    ///
    /// # Returns
    ///
    /// A new semantic convention registry.
    ///
    /// # Errors
    ///
    /// If the registry path pattern is invalid.
    pub fn try_from_path_pattern(registry_id: &str, path_pattern: &str) -> Result<Self, Error> {
        let mut registry = SemConvRegistry::new(registry_id);
        for sc_entry in glob::glob(path_pattern).map_err(|e| Error::InvalidRegistryPathPattern {
            path_pattern: path_pattern.to_owned(),
            error: e.to_string(),
        })? {
            let path_buf = sc_entry.map_err(|e| Error::InvalidRegistryPathPattern {
                path_pattern: path_pattern.to_owned(),
                error: e.to_string(),
            })?;
            let semconv_spec = SemConvSpecWithProvenance::from_file(path_buf.as_path())?;
            registry.add_semconv_spec(semconv_spec);
        }
        Ok(registry)
    }

    /// Creates a semantic convention registry from the given list of
    /// semantic convention specs.
    ///
    /// # Arguments
    ///
    /// * `registry_id` - The id of the semantic convention registry.
    /// * `semconv_specs` - The list of semantic convention specs to load.
    pub fn from_semconv_specs(
        registry_id: &str,
        semconv_specs: Vec<(String, SemConvSpec)>,
    ) -> SemConvRegistry {
        // Load all the semantic convention catalogs.
        let mut registry = SemConvRegistry::new(registry_id);

        for (provenance, spec) in semconv_specs {
            registry.add_semconv_spec(SemConvSpecWithProvenance { spec, provenance });
        }

        registry
    }

    /// Returns the id of the semantic convention registry.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Add a semantic convention spec to the semantic convention registry.
    ///
    /// # Arguments
    ///
    /// * `spec` - The semantic convention spec with provenance to add.
    fn add_semconv_spec(&mut self, spec: SemConvSpecWithProvenance) {
        self.specs.push(spec);
        self.semconv_spec_count += 1;
    }

    /// Load and add a semantic convention file to the semantic convention registry.
    pub fn add_semconv_spec_from_file<P: AsRef<Path> + Clone>(
        &mut self,
        path: P,
    ) -> Result<(), Error> {
        self.add_semconv_spec(SemConvSpecWithProvenance::from_file(path.clone())?);
        Ok(())
    }

    /// Load and add a semantic convention string to the semantic convention registry.
    pub fn add_semconv_spec_from_string(
        &mut self,
        provenance: &str,
        spec: &str,
    ) -> Result<(), Error> {
        self.add_semconv_spec(SemConvSpecWithProvenance::from_string(provenance, spec)?);
        Ok(())
    }

    /// Loads and returns the semantic convention spec from a file.
    pub fn semconv_spec_from_file<P: AsRef<Path>>(
        semconv_path: P,
    ) -> Result<(String, SemConvSpec), Error> {
        let provenance = semconv_path.as_ref().display().to_string();
        let spec = SemConvSpec::from_file(semconv_path)?;
        Ok((provenance, spec))
    }

    /// Downloads and returns the semantic convention spec from an URL.
    pub fn semconv_spec_from_url(sem_conv_url: &str) -> Result<(String, SemConvSpec), Error> {
        let spec = SemConvSpec::from_url(sem_conv_url)?;
        Ok((sem_conv_url.to_owned(), spec))
    }

    /// Returns the number of semantic convention specs added in the semantic
    /// convention registry.
    #[must_use]
    pub fn semconv_spec_count(&self) -> usize {
        self.semconv_spec_count
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
        for SemConvSpecWithProvenance { spec, provenance } in self.specs.clone() {
            for group in spec.groups.iter() {
                // Process attributes
                match group.r#type {
                    GroupType::AttributeGroup
                    | GroupType::Span
                    | GroupType::Resource
                    | GroupType::Metric
                    | GroupType::Event
                    | GroupType::Scope
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
                            GroupType::Scope => Some(&mut self.scope_group_attributes),
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
                }

                // Process metrics
                match group.r#type {
                    GroupType::Metric => {
                        // JUSTIFICATION: The expect is safe here because the validation of the
                        // metric group is done in the validation step performed at the creation
                        // of each SemConvSpec.
                        let metric_name = group.metric_name.clone().expect("At this point, the metric name should be defined.");
                        let instrument = group.instrument.clone().expect("At this point, the instrument should be defined.");

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
        for attr_to_resolve in attributes_to_resolve {
            let resolved_attr = self.all_attributes.get(&attr_to_resolve.r#ref);

            if resolved_attr.is_none() {
                let err = Error::InvalidAttribute {
                    path_or_url: attr_to_resolve.path_or_url.clone(),
                    group_id: attr_to_resolve.group_id.clone(),
                    attribute_id: attr_to_resolve.r#ref.clone(),
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

    /// Returns a map of all the attributes defined in a group given its id and type.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The id of the group.
    /// * `group_type` - The type of the group.
    ///
    /// # Returns
    ///
    /// A map of all the attributes defined in the group.
    /// If the group does not exist, an [`Error::AttributeNotFound`] is returned.
    pub fn attributes(
        &self,
        group_id: &str,
        group_type: GroupType,
    ) -> Result<HashMap<&String, &AttributeSpec>, Error> {
        let mut attributes = HashMap::new();
        let group_ids = match group_type {
            GroupType::AttributeGroup => self.attr_grp_group_attributes.get(group_id),
            GroupType::Span => self.span_group_attributes.get(group_id),
            GroupType::Event => self.event_group_attributes.get(group_id),
            GroupType::Metric => self.metric_group_attributes.get(group_id),
            GroupType::MetricGroup => self.metric_group_group_attributes.get(group_id),
            GroupType::Resource => self.resource_group_attributes.get(group_id),
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
                r#ref: group_id.to_owned(),
            });
        }
        Ok(attributes)
    }

    /// Returns an iterator over all the unresolved groups defined in the semantic convention
    /// registry.
    ///
    /// Note: This method doesn't return any group after the `resolve` method has been called.
    pub fn unresolved_group_iter(&self) -> impl Iterator<Item = &GroupSpec> {
        self.specs
            .iter()
            .flat_map(|SemConvSpecWithProvenance { spec, .. }| &spec.groups)
    }

    /// Returns an iterator over all the unresolved groups defined in the semantic convention
    /// registry. Each group is associated with its provenance (path or URL).
    ///
    /// Note: This method doesn't return any group after the `resolve` method has been called.
    pub fn unresolved_group_with_provenance_iter(
        &self,
    ) -> impl Iterator<Item = GroupSpecWithProvenance> + '_ {
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
    ///
    /// Note: This method doesn't return any attribute before the `resolve` method has been called.
    pub fn attribute_iter(&self) -> impl Iterator<Item = &AttributeSpec> {
        self.all_attributes.values().map(|attr| &attr.attribute)
    }

    /// Returns an iterator over all the metrics defined in the semantic convention registry.
    ///
    /// Note: This method doesn't return any metric before the `resolve` method has been called.
    pub fn metric_iter(&self) -> impl Iterator<Item = &MetricSpec> {
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
        for mut attr in attrs {
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
                        id.clone_from(&fq_attr_id);
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

#[cfg(test)]
mod tests {
    use crate::attribute::{AttributeSpec, AttributeType, PrimitiveOrArrayTypeSpec};
    use crate::group::{GroupSpec, GroupType};
    use crate::registry::SemConvRegistry;
    use crate::Error;

    #[test]
    fn try_from_path_pattern() {
        // Test with a valid path pattern
        let registry = SemConvRegistry::try_from_path_pattern("test", "data/c*.yaml").unwrap();
        assert_eq!(registry.id(), "test");
        assert_eq!(registry.semconv_spec_count(), 3);

        // Test with an invalid path pattern
        let registry = SemConvRegistry::try_from_path_pattern("test", "data/c***.yml");
        assert!(registry.is_err());
        assert!(matches!(
            registry.unwrap_err(),
            Error::InvalidRegistryPathPattern { .. }
        ));
    }

    #[test]
    fn test_from_semconv_specs() {
        let semconv_specs = vec![
            (
                "data/c1.yaml".to_owned(),
                super::SemConvSpec {
                    groups: vec![GroupSpec {
                        id: "group1".to_owned(),
                        r#type: GroupType::AttributeGroup,
                        attributes: vec![AttributeSpec::Id {
                            id: "attr1".to_owned(),
                            r#type: AttributeType::PrimitiveOrArray(
                                PrimitiveOrArrayTypeSpec::Boolean,
                            ),
                            brief: None,
                            examples: None,
                            tag: None,
                            requirement_level: Default::default(),
                            sampling_relevant: None,
                            note: "note".to_owned(),
                            stability: None,
                            deprecated: None,
                        }],
                        constraints: vec![],
                        span_kind: None,
                        prefix: "".to_owned(),
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        extends: None,
                        stability: None,
                        deprecated: None,
                        events: vec![],
                        name: None,
                    }],
                },
            ),
            (
                "data/c2.yaml".to_owned(),
                super::SemConvSpec {
                    groups: vec![GroupSpec {
                        id: "group2".to_owned(),
                        r#type: GroupType::AttributeGroup,
                        attributes: vec![],
                        constraints: vec![],
                        span_kind: None,
                        prefix: "".to_owned(),
                        metric_name: None,
                        instrument: None,
                        unit: None,
                        brief: "brief".to_owned(),
                        note: "note".to_owned(),
                        extends: None,
                        stability: None,
                        deprecated: None,
                        events: vec![],
                        name: None,
                    }],
                },
            ),
        ];
        let registry = SemConvRegistry::from_semconv_specs("test", semconv_specs);
        assert_eq!(registry.id(), "test");
        assert_eq!(registry.semconv_spec_count(), 2);
    }

    #[test]
    fn test_new_semconv_registry() {
        let registry = SemConvRegistry::new("test");
        assert_eq!(registry.id(), "test");
        assert_eq!(registry.semconv_spec_count(), 0);
    }

    #[test]
    fn test_semconv_from_path_pattern() {
        let mut registry = SemConvRegistry::try_from_path_pattern("test", "data/c*.yaml").unwrap();
        assert_eq!(registry.id(), "test");
        assert_eq!(registry.semconv_spec_count(), 3);

        registry
            .add_semconv_spec_from_file("data/database.yaml")
            .unwrap();
        assert_eq!(registry.semconv_spec_count(), 4);
    }

    #[test]
    fn test_stats() {
        let registry = SemConvRegistry::try_from_path_pattern("test", "data/c*.yaml").unwrap();
        let stats = registry.stats();
        assert_eq!(stats.file_count, 3);
        assert_eq!(stats.group_count, 3);
        stats
            .group_breakdown
            .iter()
            .for_each(|(group_type, total)| match group_type {
                GroupType::AttributeGroup => assert_eq!(*total, 1),
                GroupType::MetricGroup => assert_eq!(*total, 0),
                GroupType::Resource => assert_eq!(*total, 1),
                GroupType::Span => assert_eq!(*total, 1),
                _ => panic!("Unexpected group type {:?}", group_type),
            });
    }

    #[test]
    fn test_attributes() {
        let mut registry = SemConvRegistry::try_from_path_pattern("test", "data/c*.yaml").unwrap();
        let warnings = registry.resolve(Default::default()).unwrap();
        assert_eq!(warnings.len(), 0);

        // Test with valid group ids
        let attributes = registry.attributes("client", GroupType::AttributeGroup);
        assert!(attributes.is_ok());
        let attributes = attributes.unwrap();
        assert_eq!(attributes.len(), 4);
        let attributes = registry.attributes("cloud", GroupType::Resource);
        assert!(attributes.is_ok());
        let attributes = attributes.unwrap();
        assert_eq!(attributes.len(), 6);

        let attributes = registry.attributes("cloudevents", GroupType::Span);
        assert!(attributes.is_ok());
        let attributes = attributes.unwrap();
        assert_eq!(attributes.len(), 5);
        let attributes = registry.attributes("cloud", GroupType::Resource);

        assert!(attributes.is_ok());
        let attributes = attributes.unwrap();
        assert_eq!(attributes.len(), 6);

        // Test with invalid group ids
        let attributes = registry.attributes("invalid", GroupType::Metric);
        assert!(attributes.is_err());
        assert!(matches!(
            attributes.unwrap_err(),
            Error::AttributeNotFound { .. }
        ));

        let attributes = registry.attributes("invalid", GroupType::MetricGroup);
        assert!(attributes.is_err());
        assert!(matches!(
            attributes.unwrap_err(),
            Error::AttributeNotFound { .. }
        ));

        let attributes = registry.attributes("invalid", GroupType::Event);
        assert!(attributes.is_err());
        assert!(matches!(
            attributes.unwrap_err(),
            Error::AttributeNotFound { .. }
        ));
    }

    #[test]
    fn test_unresolved_group_iter() {
        let mut registry = SemConvRegistry::try_from_path_pattern("test", "data/c*.yaml").unwrap();

        let groups = registry.unresolved_group_iter().collect::<Vec<_>>();
        assert_eq!(groups.len(), 3);

        // After resolution there should be no unresolved groups
        let warnings = registry.resolve(Default::default()).unwrap();
        assert_eq!(warnings.len(), 0);
        let groups = registry.unresolved_group_iter().collect::<Vec<_>>();
        assert_eq!(groups.len(), 0);
    }

    #[test]
    fn test_unresolved_group_with_provenance_iter() {
        let mut registry = SemConvRegistry::try_from_path_pattern("test", "data/c*.yaml").unwrap();

        let groups = registry
            .unresolved_group_with_provenance_iter()
            .collect::<Vec<_>>();
        assert_eq!(groups.len(), 3);

        // After resolution there should be no unresolved groups
        let warnings = registry.resolve(Default::default()).unwrap();
        assert_eq!(warnings.len(), 0);
        let groups = registry
            .unresolved_group_with_provenance_iter()
            .collect::<Vec<_>>();
        assert_eq!(groups.len(), 0);
    }

    #[test]
    fn test_attribute_iter() {
        let mut registry = SemConvRegistry::try_from_path_pattern("test", "data/c*.yaml").unwrap();

        // Before resolution there should be no attribute returned
        // by the attribute_iter method.
        let attributes = registry.attribute_iter().collect::<Vec<_>>();
        assert_eq!(attributes.len(), 0);

        let warnings = registry.resolve(Default::default()).unwrap();
        assert_eq!(warnings.len(), 0);

        // After resolution there should be 15 attributes returned
        let attributes = registry.attribute_iter().collect::<Vec<_>>();
        assert_eq!(attributes.len(), 15);
    }

    #[test]
    fn test_metric_iter() {
        let mut registry = SemConvRegistry::try_from_path_pattern("test", "data/d*.yaml").unwrap();

        // Before resolution there should be no metric returned
        // by the metric_iter method.
        let metrics = registry.metric_iter().collect::<Vec<_>>();
        assert_eq!(metrics.len(), 0);

        _ = registry.resolve(Default::default()).unwrap();

        // After resolution there should be 9 metrics returned
        let metrics = registry.metric_iter().collect::<Vec<_>>();
        assert_eq!(metrics.len(), 9);
    }

    #[test]
    fn test_detect_duplicate_attribute_id() {
        let mut registry = SemConvRegistry::new("test");

        // Add 2 times the same semconv spec
        let result = registry.add_semconv_spec_from_file("data/client.yaml");
        assert!(result.is_ok());
        let result = registry.add_semconv_spec_from_file("data/client.yaml");
        assert!(result.is_ok());

        // Resolve the registry
        let result = registry.resolve(Default::default());
        assert!(matches!(
            result.unwrap_err(),
            Error::DuplicateAttributeId { .. }
        ));
    }

    #[test]
    fn test_detect_duplicate_group_id() {
        let mut registry = SemConvRegistry::new("test");

        // Add two semconv spec with the same group id and prefix
        let result = registry.add_semconv_spec_from_file("data/client.yaml");
        assert!(result.is_ok());

        let spec = r#"
        groups:
          - id: client
            prefix: client
            type: attribute_group
            brief: "client"
            attributes:
              - id: "attr1"
                brief: "description1"
                type: "string"
                examples: "example1"
        "#;
        let result = registry.add_semconv_spec_from_string("data/client_bis.yaml", spec);
        assert!(result.is_ok());

        // The resolution of the registry should fail with a duplicate group id error
        let result = registry.resolve(Default::default());
        assert!(matches!(
            result.unwrap_err(),
            Error::DuplicateGroupId { .. }
        ));
    }

    #[test]
    fn test_metrics() {
        let mut registry = SemConvRegistry::new("test");

        // Add a valid semconv spec with a metric
        let spec = r#"
        groups:
          - id: metric_group
            type: metric
            brief: "metric"
            attributes:
              - id: "attr1"
                brief: "description1"
                type: "string"
                examples: "example1"
            metric_name: "metric1"
            instrument: "counter"
            unit: "unit1"
        "#;
        let result = registry.add_semconv_spec_from_string("data/metric.yaml", spec);
        assert!(result.is_ok());

        // Resolve the registry
        let result = registry.resolve(Default::default());
        assert!(result.is_ok());
        let warnings = result.unwrap();
        assert_eq!(warnings.len(), 0);

        let mut registry = SemConvRegistry::new("test");

        // Add an invalid semconv spec with a metric
        // The metric has no name
        let spec = r#"
        groups:
          - id: metric_group
            type: metric
            brief: "metric"
            attributes:
              - id: "attr1"
                brief: "description1"
                type: "string"
                examples: "example1"
            instrument: "counter"
            unit: "unit1"
        "#;
        let result = registry.add_semconv_spec_from_string("data/metric.yaml", spec);
        dbg!(&result);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::InvalidMetric { .. }
        ));

        // Add an invalid semconv spec with a metric
        // The metric has no instrument
        let spec = r#"
        groups:
          - id: metric_group
            type: metric
            brief: "metric"
            attributes:
              - id: "attr1"
                brief: "description1"
                type: "string"
                examples: "example1"
            metric_name: "metric1"
            unit: "unit1"
        "#;
        let result = registry.add_semconv_spec_from_string("data/metric.yaml", spec);
        dbg!(&result);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::InvalidMetric { .. }
        ));

        // ToDo collect multiple errors
    }
}
