// SPDX-License-Identifier: Apache-2.0

//! Set of utility filters and tests used by the Weaver project.

use crate::config::WeaverConfig;
use crate::v2::entity::EntityRef;
use crate::v2::registry::ForgeResolvedRegistry;
use minijinja::value::Rest;
use minijinja::{Environment, ErrorKind, Value};
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// Add utility filters and tests to the environment.
pub(crate) fn add_filters(env: &mut Environment<'_>, target_config: &WeaverConfig) {
    env.add_filter(
        "acronym",
        acronym(target_config.acronyms.clone().unwrap_or_default()),
    );
    env.add_filter("flatten", flatten);
    env.add_filter("numsort", numsort);
    env.add_filter("split_id", split_id);
    env.add_filter("regex_replace", regex_replace);
    env.add_filter("toyaml", to_yaml);
}

/// Add utility functions to the environment.
///
/// `registry` is the v2 registry under generation, if there is one.
/// `lookup_entity` reads entity definitions from it.
pub(crate) fn add_functions(
    env: &mut Environment<'_>,
    registry: Option<Arc<ForgeResolvedRegistry>>,
) {
    env.add_function("concat_if", concat_if);
    env.add_function("lookup_entity", lookup_entity(registry));
}

/// Returns a function that reads the entity definition an
/// `entity_associations` leaf names.
///
/// A leaf names the entity and the registry that defines it, so the lookup
/// reads one registry. A registry does not copy the entities of its
/// dependencies, so a leaf often names a dependency.
fn lookup_entity(
    registry: Option<Arc<ForgeResolvedRegistry>>,
) -> impl Fn(Value) -> Result<Value, minijinja::Error> {
    move |leaf: Value| {
        let Some(registry) = registry.as_deref() else {
            return Err(minijinja::Error::new(
                ErrorKind::InvalidOperation,
                "`lookup_entity` needs a v2 registry, and this target generates from another context",
            ));
        };
        // The leaf is plain data in the template. Serde reads it back into the
        // type the lookup takes, so the shape is defined in one place.
        let entity_ref: EntityRef = serde_json::to_value(&leaf)
            .and_then(serde_json::from_value)
            .map_err(|e| {
                minijinja::Error::new(
                    ErrorKind::InvalidOperation,
                    format!("`lookup_entity` expects an entity association reference, found `{leaf}`: {e}"),
                )
            })?;
        registry
            .lookup_entity(&entity_ref)
            .map(Value::from_serialize)
            .map_err(|e| minijinja::Error::new(ErrorKind::InvalidOperation, e.to_string()))
    }
}

/// Concatenate a list of values into a single string IF all values are defined.
/// If any value is undefined, the filter will return an undefined value.
fn concat_if(args: Rest<Value>) -> Value {
    let mut result = String::new();
    for arg in args.iter() {
        if arg.is_undefined() {
            return Value::default();
        }
        result.push_str(arg.to_string().as_str());
    }
    Value::from(result)
}

// Helper filter to work around lack of `list.append()` support in minijinja.
// Will take a list of lists and return a new list containing only elements of sublists.
fn flatten(value: Value) -> Result<Value, minijinja::Error> {
    let mut result = Vec::new();
    for sublist in value.try_iter()? {
        for item in sublist.try_iter()? {
            result.push(item);
        }
    }
    Ok(Value::from(result))
}

/// Sort a map's entries by numeric key value, returning `[(key, value), ...]`.
/// Keys are parsed as integers; non-numeric keys are sorted after numeric ones.
fn numsort(value: Value) -> Result<Value, minijinja::Error> {
    fn parse_num(v: &Value) -> Option<i64> {
        v.as_i64()
            .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
    }

    let mut pairs: Vec<(Value, Value)> = Vec::new();
    for key in value.try_iter()? {
        let val = value.get_item(&key)?;
        pairs.push((key, val));
    }
    pairs.sort_by(|a, b| {
        let a_num = parse_num(&a.0);
        let b_num = parse_num(&b.0);
        match (a_num, b_num) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.0.to_string().cmp(&b.0.to_string()),
        }
    });
    Ok(Value::from(
        pairs
            .into_iter()
            .map(|(k, v)| Value::from(vec![k, v]))
            .collect::<Vec<_>>(),
    ))
}

// Helper function to take an "id" and split it by '.' into namespaces.
fn split_id(value: Value) -> Result<Vec<Value>, minijinja::Error> {
    match value.as_str() {
        Some(id) => {
            let values: Vec<Value> = id
                .split('.')
                .map(|s| Value::from_safe_string(s.to_owned()))
                .collect();
            Ok(values)
        }
        None => Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("Expected string, found: {value}"),
        )),
    }
}

/// Replace all occurrences of a regex pattern (1st parameter) in the input string with the
/// replacement string (2nd parameter).
fn regex_replace(
    input: Cow<'_, str>,
    pattern: Cow<'_, str>,
    replacement: Cow<'_, str>,
) -> Result<String, minijinja::Error> {
    let re = Regex::new(pattern.as_ref()).map_err(|e| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("Invalid regex pattern: {e}"),
        )
    })?;
    Ok(re
        .replace_all(input.as_ref(), replacement.as_ref())
        .to_string())
}

/// Create a filter that replaces acronyms in the input string with the full
/// name defined in the `acronyms` list.
///
/// Note: Whitespace and punctuation are preserved.
///
/// # Arguments
///
/// * `acronyms` - A list of acronyms to replace in the input string.
///
/// # Example
///
/// ```rust
/// use weaver_forge::extensions::util;
///
/// let acronyms = vec!["iOS".to_owned(), "API".to_owned(), "URL".to_owned()];
/// let filter = util::acronym(acronyms);
///
/// assert_eq!(filter("This is an - IOS - device!"), "This is an - iOS - device!");
/// assert_eq!(filter("This is another type of api with the following url!   "), "This is another type of API with the following URL!   ");
/// ```
///
/// # Returns
///
/// A function that takes an input string and returns a new string with the
/// acronyms replaced.
pub fn acronym(acronyms: Vec<String>) -> impl Fn(&str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let acronym_map = acronyms
        .iter()
        .map(|acronym| (acronym.to_lowercase(), acronym.clone()))
        .collect::<HashMap<String, String>>();

    move |input: &str| -> String {
        // Pattern to match sequences of whitespace (\s+), non-whitespace
        // non-punctuation (\w+), or any punctuation ([^\w\s]+)
        let re = RE.get_or_init(|| Regex::new(r"(\s+|\w+|[^\w\s]+)").expect("Invalid regex"));
        re.find_iter(input)
            .map(|mat| match acronym_map.get(&mat.as_str().to_lowercase()) {
                Some(acronym) => acronym.clone(),
                None => mat.as_str().to_owned(),
            })
            .collect()
    }
}

// Helper filter to dump value (1st parameter) in yaml format.
fn to_yaml(value: &Value) -> Result<Value, minijinja::Error> {
    let yaml = serde_yaml::to_string(&value)
        .map_err(|e| minijinja::Error::new(ErrorKind::BadSerialization, e.to_string()))
        .map(|s| {
            // When this filter is used the return value is safe for both HTML and JSON
            let mut rv = String::with_capacity(s.len());
            for c in s.chars() {
                match c {
                    '<' => rv.push_str("\\u003c"),
                    '>' => rv.push_str("\\u003e"),
                    '&' => rv.push_str("\\u0026"),
                    '\'' => rv.push_str("\\u0027"),
                    _ => rv.push(c),
                }
            }
            Value::from_safe_string(rv)
        })?;
    Ok(yaml)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use crate::extensions::util::{add_filters, add_functions};
    use crate::v2::attribute::Attribute;
    use crate::v2::entity::{Entity, EntityAttribute, EntityRefinement};
    use crate::v2::provenance::Provenance;
    use crate::v2::registry::{ForgeResolvedRegistry, Refinements, Registry};
    use minijinja::Environment;
    use serde_yaml::{Mapping, Number, Value};
    use weaver_semconv::attribute::{
        AttributeType, BasicRequirementLevelSpec, PrimitiveOrArrayTypeSpec, RequirementLevel,
    };
    use weaver_semconv::v2::CommonFields;

    /// An identity attribute for a test entity.
    fn test_attribute(key: &str) -> EntityAttribute {
        EntityAttribute {
            base: Attribute {
                key: key.to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                examples: None,
                common: CommonFields::default(),
                provenance: Provenance::default(),
            },
            requirement_level: RequirementLevel::Basic(BasicRequirementLevelSpec::Required),
        }
    }

    /// A test entity. Its identity attribute key names the registry that
    /// defines it, so a test can tell two entities of one type apart.
    fn test_entity(r#type: &str, identity_key: &str) -> Entity {
        Entity {
            r#type: r#type.to_owned().into(),
            identity: vec![test_attribute(identity_key)],
            description: vec![],
            requirement_level: None,
            common: CommonFields::default(),
            provenance: Provenance::default(),
        }
    }

    /// A registry that defines `host` and refines it as `host.linux`. It
    /// depends on a registry that defines its own `host` and a `deployment`.
    fn test_registry() -> ForgeResolvedRegistry {
        let dependency = ForgeResolvedRegistry {
            schema_url: "https://example.com/base/1.0.0"
                .try_into()
                .expect("a valid schema url"),
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: vec![
                    test_entity("host", "base.host.name"),
                    test_entity("deployment", "base.deployment.name"),
                ],
            },
            refinements: Refinements {
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: vec![],
            },
            dependencies: vec![],
        };
        ForgeResolvedRegistry {
            schema_url: "https://example.com/main/1.0.0"
                .try_into()
                .expect("a valid schema url"),
            registry: Registry {
                attributes: vec![],
                attribute_groups: vec![],
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: vec![test_entity("host", "main.host.name")],
            },
            refinements: Refinements {
                metrics: vec![],
                spans: vec![],
                events: vec![],
                entities: vec![EntityRefinement {
                    id: "host.linux".to_owned().into(),
                    entity: test_entity("host", "main.host.linux.name"),
                }],
            },
            dependencies: vec![dependency],
        }
    }

    /// An environment where `lookup_entity` reads [`test_registry`].
    fn env_with_registry() -> Environment<'static> {
        let mut env = Environment::new();
        add_functions(&mut env, Some(Arc::new(test_registry())));
        env
    }

    /// Renders the identity attribute keys of the entity that a leaf names.
    const IDENTITY: &str =
        "{% for attr in lookup_entity(ctx).identity %}{{ attr.key }}{% endfor %}";

    /// A leaf with no provenance names an entity of this registry.
    #[test]
    fn test_lookup_entity_local() {
        let ctx = serde_json::json!({"ctx": {"type": "host"}});
        assert_eq!(
            env_with_registry()
                .render_str(IDENTITY, &ctx)
                .expect("the local entity"),
            "main.host.name"
        );
    }

    /// A leaf that names a dependency reads the definition from there. This
    /// registry holds no copy of it.
    #[test]
    fn test_lookup_entity_from_dependency() {
        let ctx = serde_json::json!({
            "ctx": {
                "type": "deployment",
                "provenance": {"source": "https://example.com/base/1.0.0"},
            }
        });
        assert_eq!(
            env_with_registry()
                .render_str(IDENTITY, &ctx)
                .expect("the entity of the dependency"),
            "base.deployment.name"
        );
    }

    /// Two registries define `host`. Each leaf reads the definition from the
    /// registry it names.
    #[test]
    fn test_lookup_entity_reads_the_registry_the_leaf_names() {
        let ctx = serde_json::json!({
            "ctx": {
                "type": "host",
                "provenance": {"source": "https://example.com/base/1.0.0"},
            }
        });
        assert_eq!(
            env_with_registry()
                .render_str(IDENTITY, &ctx)
                .expect("the host of the dependency"),
            "base.host.name"
        );
    }

    /// A leaf names an entity type or the id of an entity refinement. The
    /// lookup finds both.
    #[test]
    fn test_lookup_entity_refinement() {
        let ctx = serde_json::json!({"ctx": {"type": "host.linux"}});
        assert_eq!(
            env_with_registry()
                .render_str(IDENTITY, &ctx)
                .expect("the refinement"),
            "main.host.linux.name"
        );
    }

    /// The whole definition is available, not the identity alone.
    #[test]
    fn test_lookup_entity_returns_the_definition() {
        let ctx = serde_json::json!({"ctx": {"type": "host"}});
        assert_eq!(
            env_with_registry()
                .render_str("{{ lookup_entity(ctx).type }}", &ctx)
                .expect("the local entity"),
            "host"
        );
    }

    /// A name that no registry defines is an error, and the error names it.
    #[test]
    fn test_lookup_entity_not_found() {
        let ctx = serde_json::json!({"ctx": {"type": "nothing"}});
        let error = env_with_registry()
            .render_str(IDENTITY, &ctx)
            .expect_err("no such entity");
        assert!(
            error.to_string().contains("Entity `nothing` was not found"),
            "unexpected error: {error}"
        );
    }

    /// A `one_of` or `all_of` node is not a leaf. The function says so instead
    /// of rendering nothing.
    #[test]
    fn test_lookup_entity_rejects_an_association_tree() {
        let ctx = serde_json::json!({"ctx": {"one_of": [{"type": "host"}]}});
        let error = env_with_registry()
            .render_str(IDENTITY, &ctx)
            .expect_err("not a reference");
        assert!(
            error
                .to_string()
                .contains("expects an entity association reference"),
            "unexpected error: {error}"
        );
    }

    /// Without a v2 registry there are no definitions to read.
    #[test]
    fn test_lookup_entity_without_a_v2_registry() {
        let mut env = Environment::new();
        add_functions(&mut env, None);
        let ctx = serde_json::json!({"ctx": {"type": "host"}});
        let error = env
            .render_str(IDENTITY, &ctx)
            .expect_err("no registry to read");
        assert!(
            error.to_string().contains("needs a v2 registry"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn test_regex_replace() {
        let mut env = Environment::new();
        let ctx = serde_json::Value::Null;
        let config = crate::config::WeaverConfig::default();

        add_filters(&mut env, &config);

        assert_eq!(
            env.render_str("{{ 'Hello World!' | regex_replace('!','?') }}", &ctx)
                .unwrap(),
            "Hello World?"
        );

        assert_eq!(
            env.render_str(
                "{{ \"This a test with multiple a's\" | regex_replace('a','A') }}",
                &ctx
            )
            .unwrap(),
            "This A test with multiple A's"
        );
    }
    #[test]
    fn test_to_yaml() {
        let mut env = Environment::new();
        let mut inner_map = Mapping::new();
        let _ = inner_map.insert(
            Value::String("age".to_owned()),
            Value::Number(Number::from(30u64)),
        );
        let mut details_map = Mapping::new();
        let _ = details_map.insert(
            Value::String("city".to_owned()),
            Value::String("Wonderland".to_owned()),
        );
        let _ = details_map.insert(
            Value::String("email".to_owned()),
            Value::String("alice@example.com".to_owned()),
        );
        let _ = inner_map.insert(
            Value::String("details".to_owned()),
            Value::Mapping(details_map),
        );
        let _ = inner_map.insert(Value::String("is_active".to_owned()), Value::Bool(true));
        let _ = inner_map.insert(
            Value::String("name".to_owned()),
            Value::String("Alice".to_owned()),
        );
        let _ = inner_map.insert(
            Value::String("about".to_owned()),
            Value::String("<h1>Software Engineer<h1>".to_owned()),
        );
        let _ = inner_map.insert(
            Value::String("profile".to_owned()),
            Value::String("https://example.com/?page=1&section=about".to_owned()),
        );
        let _ = inner_map.insert(
            Value::String("skills".to_owned()),
            Value::Sequence(vec![
                Value::String("Rust".to_owned()),
                Value::String("JavaScript".to_owned()),
            ]),
        );
        let mut map = Mapping::new();
        let _ = map.insert(Value::String("user".to_owned()), Value::Mapping(inner_map));

        let ctx = Value::Mapping(map);
        let config = crate::config::WeaverConfig::default();

        add_filters(&mut env, &config);
        let expected_yaml = fs::read_to_string("expected_output/yaml/test.yaml").unwrap();
        // Normalize line endings for both strings (remove any \r characters)
        let normalized_expected = expected_yaml.replace("\r\n", "\n");
        let normalized_actual = env
            .render_str("{{ user | toyaml }}", &ctx)
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(normalized_actual, normalized_expected);
    }

    #[test]
    fn test_numsort_numeric_string_keys() {
        let mut env = Environment::new();
        let config = crate::config::WeaverConfig::default();
        add_filters(&mut env, &config);

        // BTreeMap<usize, usize> serializes with string keys in JSON.
        // numsort should order them numerically, not lexicographically.
        let ctx: serde_json::Value = serde_json::json!({
            "m": {"1": 10, "10": 20, "2": 30, "20": 40, "3": 50}
        });
        assert_eq!(
            env.render_str(
                "{% for k, v in m | numsort %}{{ k }}:{{ v }}{% if not loop.last %},{% endif %}{% endfor %}",
                &ctx,
            )
            .unwrap(),
            "1:10,2:30,3:50,10:20,20:40"
        );
    }

    #[test]
    fn test_numsort_mixed_keys() {
        let mut env = Environment::new();
        let config = crate::config::WeaverConfig::default();
        add_filters(&mut env, &config);

        // Non-numeric keys should sort after numeric keys, alphabetically.
        let ctx: serde_json::Value = serde_json::json!({
            "m": {"3": "c", "alpha": "a", "1": "b", "beta": "d"}
        });
        assert_eq!(
            env.render_str(
                "{% for k, v in m | numsort %}{{ k }}:{{ v }}{% if not loop.last %},{% endif %}{% endfor %}",
                &ctx,
            )
            .unwrap(),
            "1:b,3:c,alpha:a,beta:d"
        );
    }

    #[test]
    fn test_numsort_empty_map() {
        let mut env = Environment::new();
        let config = crate::config::WeaverConfig::default();
        add_filters(&mut env, &config);

        let ctx: serde_json::Value = serde_json::json!({"m": {}});
        assert_eq!(
            env.render_str("{% for k, v in m | numsort %}{{ k }}{% endfor %}", &ctx,)
                .unwrap(),
            ""
        );
    }
}
