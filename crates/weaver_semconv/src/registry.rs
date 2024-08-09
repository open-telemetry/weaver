// SPDX-License-Identifier: Apache-2.0

//! Semantic Convention Registry.

use crate::attribute::AttributeSpecWithProvenance;
use crate::group::GroupSpecWithProvenance;
use crate::metric::MetricSpecWithProvenance;
use crate::semconv::{SemConvSpec, SemConvSpecWithProvenance};
use crate::stats::Stats;
use crate::Error;
use std::collections::HashMap;
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
    attributes: HashMap<String, AttributeSpecWithProvenance>,

    /// Metrics indexed by their respective id.
    ///
    /// This collection contains all the metrics defined in the semantic convention registry.
    metrics: HashMap<String, MetricSpecWithProvenance>,
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
    pub fn try_from_path_pattern(
        registry_id: &str,
        path_pattern: &str,
        strict_mode: bool,
    ) -> Result<Self, Error> {
        let mut registry = SemConvRegistry::new(registry_id);
        for sc_entry in glob::glob(path_pattern).map_err(|e| Error::InvalidRegistryPathPattern {
            path_pattern: path_pattern.to_owned(),
            error: e.to_string(),
        })? {
            let path_buf = sc_entry.map_err(|e| Error::InvalidRegistryPathPattern {
                path_pattern: path_pattern.to_owned(),
                error: e.to_string(),
            })?;
            let semconv_spec =
                SemConvSpecWithProvenance::from_file(path_buf.as_path(), strict_mode)?;
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
        // Load all the semantic convention registry.
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
        strict_mode: bool,
    ) -> Result<(), Error> {
        self.add_semconv_spec(SemConvSpecWithProvenance::from_file(
            path.clone(),
            strict_mode,
        )?);
        Ok(())
    }

    /// Load and add a semantic convention string to the semantic convention registry.
    pub fn add_semconv_spec_from_string(
        &mut self,
        provenance: &str,
        spec: &str,
        strict_mode: bool,
    ) -> Result<(), Error> {
        self.add_semconv_spec(SemConvSpecWithProvenance::from_string(
            provenance,
            spec,
            strict_mode,
        )?);
        Ok(())
    }

    /// Loads and returns the semantic convention spec from a file.
    pub fn semconv_spec_from_file<P: AsRef<Path>>(
        semconv_path: P,
        strict_mode: bool,
    ) -> Result<(String, SemConvSpec), Error> {
        let provenance = semconv_path.as_ref().display().to_string();
        let spec = SemConvSpec::from_file(semconv_path, strict_mode)?;
        Ok((provenance, spec))
    }

    /// Downloads and returns the semantic convention spec from an URL.
    pub fn semconv_spec_from_url(
        sem_conv_url: &str,
        strict_mode: bool,
    ) -> Result<(String, SemConvSpec), Error> {
        let spec = SemConvSpec::from_url(sem_conv_url, strict_mode)?;
        Ok((sem_conv_url.to_owned(), spec))
    }

    /// Returns the number of semantic convention specs added in the semantic
    /// convention registry.
    #[must_use]
    pub fn semconv_spec_count(&self) -> usize {
        self.semconv_spec_count
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
            attribute_count: self.attributes.len(),
            metric_count: self.metrics.len(),
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
    fn test_try_from_path_pattern() {
        // Test with a valid path pattern
        let registry =
            SemConvRegistry::try_from_path_pattern("test", "data/c*.yaml", true).unwrap();
        assert_eq!(registry.id(), "test");
        assert_eq!(registry.semconv_spec_count(), 3);

        // Test with an invalid path pattern
        let registry = SemConvRegistry::try_from_path_pattern("test", "data/c***.yml", true);
        assert!(registry.is_err());
        assert!(matches!(
            registry.unwrap_err(),
            Error::InvalidRegistryPathPattern { .. }
        ));
    }

    #[test]
    fn test_semconv_spec_from_url() {
        let semconv_url = "https://raw.githubusercontent.com/open-telemetry/semantic-conventions/main/model/url.yaml";
        let result = SemConvRegistry::semconv_spec_from_url(semconv_url, true);
        assert!(result.is_ok());
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
                        display_name: Some("Group 1".to_owned()),
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
                        display_name: Some("Group 2".to_owned()),
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
        let mut registry =
            SemConvRegistry::try_from_path_pattern("test", "data/c*.yaml", true).unwrap();
        assert_eq!(registry.id(), "test");
        assert_eq!(registry.semconv_spec_count(), 3);

        registry
            .add_semconv_spec_from_file("data/database.yaml", true)
            .unwrap();
        assert_eq!(registry.semconv_spec_count(), 4);
    }

    #[test]
    fn test_stats() {
        let registry =
            SemConvRegistry::try_from_path_pattern("test", "data/c*.yaml", true).unwrap();
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
    fn test_unresolved_group_with_provenance_iter() {
        let registry =
            SemConvRegistry::try_from_path_pattern("test", "data/c*.yaml", true).unwrap();

        let groups = registry
            .unresolved_group_with_provenance_iter()
            .collect::<Vec<_>>();
        assert_eq!(groups.len(), 3);
    }
}
