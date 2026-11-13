// SPDX-License-Identifier: Apache-2.0

//! Rego policy-based advisor

use serde::Serialize;
use serde_json::{to_value, Map, Value};
use std::{collections::BTreeMap, path::PathBuf, rc::Rc};
use weaver_checker::{Engine, InputValue, PolicyFinding};
use weaver_forge::jq;

use super::{emit_findings, Advisor};
use crate::{
    live_checker::LiveChecker, otlp_logger::OtlpEmitter,
    sample_instrumentation_scope::SampleInstrumentationScope, sample_resource::SampleResource,
    Error, Sample, SampleRef, VersionedAttribute, VersionedSignal, DEFAULT_LIVE_CHECK_JQ,
    DEFAULT_LIVE_CHECK_REGO, DEFAULT_LIVE_CHECK_REGO_POLICY_PATH,
};

/// An advisor which runs a rego policy on the attribute
#[derive(Clone)]
pub struct RegoAdvisor {
    engine: Engine,
    resource: SharedInput<SampleResource>,
    instrumentation_scope: SharedInput<SampleInstrumentationScope>,
}

impl RegoAdvisor {
    /// Create a new RegoAdvisor
    pub fn new(
        live_checker: &LiveChecker,
        policy_dir: &Option<PathBuf>,
        jq_preprocessor: &Option<PathBuf>,
        advice_data: &Option<String>,
    ) -> Result<Self, Error> {
        let mut engine = Engine::new();
        if let Some(path) = policy_dir {
            engine
                .add_policy_from_file_or_dir(path)
                .map_err(|e| Error::AdviceError {
                    error: e.to_string(),
                })?;
        } else {
            let _ = engine
                .add_policy(DEFAULT_LIVE_CHECK_REGO_POLICY_PATH, DEFAULT_LIVE_CHECK_REGO)
                .map_err(|e| Error::AdviceError {
                    error: e.to_string(),
                })?;
        }

        if let Some(pattern) = advice_data {
            engine
                .add_data_from_file_or_dir(pattern)
                .map_err(|e| Error::AdviceError {
                    error: e.to_string(),
                })?;
        }

        // If there is a jq preprocessor then pass the live_checker data through it before adding it to the engine
        // Otherwise use the default jq preprocessor
        let jq_filter = if let Some(path) = jq_preprocessor {
            std::fs::read_to_string(path).map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?
        } else {
            DEFAULT_LIVE_CHECK_JQ.to_owned()
        };

        let jq_result = jq::execute_jq(
            &to_value(live_checker).map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?,
            &jq_filter,
            &BTreeMap::new(),
        )
        .map_err(|e| Error::AdviceError {
            error: e.to_string(),
        })?;

        engine
            .add_data(&jq_result)
            .map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?;

        Ok(RegoAdvisor {
            engine,
            resource: None,
            instrumentation_scope: None,
        })
    }

    fn check(&mut self, input: InputValue) -> Result<Vec<PolicyFinding>, Error> {
        self.engine.set_input_value(input);
        let violations = self
            .engine
            .check(weaver_checker::PolicyStage::LiveCheckAdvice)
            .map_err(|e| Error::AdviceError {
                error: e.to_string(),
            })?;
        // Extract advice from violations
        Ok(violations)
    }
}

/// Attribute list fields of a sample, each with the name of its by-name map.
const ATTRIBUTE_LISTS: [(&str, &str); 2] = [
    ("attributes", "attributes_by_name"),
    ("filtered_attributes", "filtered_attributes_by_name"),
];

/// Fields that are not searched for attribute lists.
const SKIPPED_FIELDS: [&str; 3] = ["attributes", "filtered_attributes", "live_check_result"];

/// Adds a name-to-value map next to every attribute list in a serialized sample.
fn add_attributes_by_name(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object
                .iter_mut()
                .filter(|(key, _)| !SKIPPED_FIELDS.contains(&key.as_str()))
                .for_each(|(_, child)| add_attributes_by_name(child));
            for (list, by_name) in ATTRIBUTE_LISTS {
                if let Some(Value::Array(attributes)) = object.get(list) {
                    let map: Map<String, Value> = attributes
                        .iter()
                        .filter_map(|attribute| {
                            let name = attribute.get("name")?.as_str()?;
                            let value = attribute.get("value").cloned().unwrap_or(Value::Null);
                            Some((name.to_owned(), value))
                        })
                        .collect();
                    let _ = object.insert(by_name.to_owned(), Value::Object(map));
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(add_attributes_by_name),
        _ => {}
    }
}

/// Whether the by-name attribute maps are added to an input value.
#[derive(Clone, Copy, PartialEq)]
enum AttributeMaps {
    Add,
    Omit,
}

/// Converts a value for the policy input.
fn input_value<T: Serialize>(value: &T, maps: AttributeMaps) -> Result<InputValue, Error> {
    let mut json = to_value(value).map_err(|e| Error::AdviceError {
        error: e.to_string(),
    })?;
    if maps == AttributeMaps::Add {
        add_attributes_by_name(&mut json);
    }
    InputValue::from_json(json).map_err(|e| Error::AdviceError {
        error: e.to_string(),
    })
}

/// The converted input of the last shared value. The `Rc` keeps its allocation
/// alive, so pointer equality identifies it.
type SharedInput<T> = Option<(Rc<T>, InputValue)>;

/// Returns the input of `shared`, converted only when it differs from the cached one.
fn shared_input_value<T: Serialize>(
    cache: &mut SharedInput<T>,
    shared: &Rc<T>,
) -> Result<InputValue, Error> {
    if let Some((cached, input)) = cache {
        if Rc::ptr_eq(cached, shared) {
            return Ok(input.clone());
        }
    }
    let input = input_value(shared.as_ref(), AttributeMaps::Add)?;
    *cache = Some((Rc::clone(shared), input.clone()));
    Ok(input)
}

impl Advisor for RegoAdvisor {
    fn advise(
        &mut self,
        sample: SampleRef<'_>,
        signal: &Sample,
        registry_attribute: Option<Rc<VersionedAttribute>>,
        registry_group: Option<Rc<VersionedSignal>>,
        otlp_emitter: Option<Rc<OtlpEmitter>>,
    ) -> Result<Vec<PolicyFinding>, Error> {
        // An attribute sample has no attribute lists, and its value is arbitrary data.
        let sample_maps = if matches!(sample, SampleRef::Attribute(_)) {
            AttributeMaps::Omit
        } else {
            AttributeMaps::Add
        };
        let resource = match signal.shared_resource() {
            Some(shared) => shared_input_value(&mut self.resource, shared)?,
            None => input_value(&signal.resource(), AttributeMaps::Add)?,
        };
        let instrumentation_scope = match signal.shared_instrumentation_scope() {
            Some(shared) => shared_input_value(&mut self.instrumentation_scope, shared)?,
            None => InputValue::null(),
        };
        let input = InputValue::object([
            ("sample", input_value(&sample, sample_maps)?),
            ("resource", resource),
            ("instrumentation_scope", instrumentation_scope),
            (
                "registry_attribute",
                input_value(&registry_attribute, AttributeMaps::Omit)?,
            ),
            (
                "registry_group",
                input_value(&registry_group, AttributeMaps::Omit)?,
            ),
        ]);
        let mut findings = self.check(input)?;

        // Populate signal_type and signal_name from the parent signal if not already set
        for finding in &mut findings {
            if finding.signal_type.is_none() {
                finding.signal_type = signal.signal_type();
            }
            if finding.signal_name.is_none() {
                finding.signal_name = signal.signal_name();
            }
        }

        // Emit each finding if emitter available
        emit_findings(&findings, &sample, otlp_emitter.as_deref(), signal);

        Ok(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::add_attributes_by_name;
    use serde_json::json;

    #[test]
    fn adds_a_map_next_to_each_attribute_list() {
        let mut sample = json!({
            "span": {
                "name": "checkout",
                "attributes": [
                    {"name": "error.type", "value": "timeout", "type": "string"},
                    {"name": "retry.count", "value": 2},
                    {"name": "no.value"}
                ],
                "span_events": [
                    {"name": "retry", "attributes": [{"name": "attempt", "value": 1}]}
                ],
                "span_links": [{"attributes": []}]
            }
        });
        add_attributes_by_name(&mut sample);

        let span = &sample["span"];
        assert_eq!(
            span["attributes_by_name"],
            json!({"error.type": "timeout", "retry.count": 2, "no.value": null})
        );
        assert_eq!(
            span["span_events"][0]["attributes_by_name"],
            json!({"attempt": 1})
        );
        assert_eq!(span["span_links"][0]["attributes_by_name"], json!({}));
        assert_eq!(span["attributes"].as_array().map(Vec::len), Some(3));
    }

    #[test]
    fn adds_a_map_for_exemplar_filtered_attributes() {
        let mut sample = json!({
            "number_data_point": {
                "attributes": [{"name": "state", "value": "idle"}],
                "exemplars": [
                    {"filtered_attributes": [{"name": "user.id", "value": "u1"}]}
                ]
            }
        });
        add_attributes_by_name(&mut sample);

        let point = &sample["number_data_point"];
        assert_eq!(point["attributes_by_name"], json!({"state": "idle"}));
        assert_eq!(
            point["exemplars"][0]["filtered_attributes_by_name"],
            json!({"user.id": "u1"})
        );
    }

    #[test]
    fn attribute_values_and_results_are_left_unchanged() {
        let nested = json!({"attributes": [{"name": "inner", "value": 1}]});
        let mut sample = json!({
            "log": {
                "attributes": [{"name": "payload", "value": nested.clone()}],
                "live_check_result": {"all_advice": [{"context": nested.clone()}]}
            }
        });
        add_attributes_by_name(&mut sample);

        let log = &sample["log"];
        assert_eq!(log["attributes"][0]["value"], nested);
        assert_eq!(log["attributes_by_name"]["payload"], nested);
        assert_eq!(log["live_check_result"]["all_advice"][0]["context"], nested);
    }
}
