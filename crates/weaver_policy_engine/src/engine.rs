// SPDX-License-Identifier: Apache-2.0

//! Main interface for the policy engine.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use regorus::QueryResults;
use serde::{Deserialize, Serialize};
use serde_json::{to_value, Value};
use serde_json::Value::Null;

use crate::Error;

#[derive(Clone)]
pub struct Engine {
    // The policy engine.
    engine: regorus::Engine,
}

impl Engine {
    /// Creates a new policy engine.
    pub fn new() -> Self {
        Self {
            engine: regorus::Engine::new(),
        }
    }

    /// Adds a policy to the policy engine.
    ///
    /// # Arguments
    ///
    /// * `policy_path` - The path to the policy file.
    pub fn add_policy<P: AsRef<Path>>(&mut self, policy_path: P) -> Result<(), Error> {
        let policy_path_str = policy_path.as_ref().to_string_lossy().to_string();

        self.engine.add_policy_from_file(policy_path).map_err(|e| Error::InvalidPolicyFile {
            file: policy_path_str.clone(),
            error: e.to_string(),
        })
    }

    /// Adds a data document to the policy engine.
    ///
    /// Data versus Input: In essence, data is about what the policy engine
    /// knows globally and statically (or what is updated dynamically but
    /// considered part of policy engine's world knowledge), while input is
    /// about what each request or query brings to the policy engine at
    /// runtime, needing a decision based on current, external circumstances.
    /// Combining data and input allows the policy engine to make informed,
    /// context-aware decisions based on both its internal knowledge base and
    /// the specifics of each request or action being evaluated.
    pub fn add_data<T: Serialize>(&mut self, data: &T) -> Result<(), Error> {
        let json_data = to_value(data).map_err(|e| Error::InvalidData {
            error: e.to_string(),
        })?;
        self.engine.add_data(convert(json_data)).map_err(|e| Error::InvalidData {
            error: e.to_string(),
        })
    }

    /// Sets an input document for the policy engine.
    ///
    /// Data versus Input: In essence, data is about what the policy engine
    /// knows globally and statically (or what is updated dynamically but
    /// considered part of policy engine's world knowledge), while input is
    /// about what each request or query brings to the policy engine at
    /// runtime, needing a decision based on current, external circumstances.
    /// Combining data and input allows the policy engine to make informed,
    /// context-aware decisions based on both its internal knowledge base and
    /// the specifics of each request or action being evaluated.
    pub fn set_input<T: Serialize>(&mut self, input: &T) -> Result<(), Error> {
        let json_input = to_value(input).map_err(|e| Error::InvalidInput {
            error: e.to_string(),
        })?;
        self.engine.set_input(convert(json_input));
        Ok(())
    }

    /// Evaluates a Rego query.
    pub fn eval_query(&mut self, query: String) -> Result<QueryResults, Error> {
        self.engine.eval_query(query, false).map_err(|e| Error::QueryEvaluationError {
            error: e.to_string(),
        })
    }

    /// Evaluates a Rego query and returns a boolean result.
    pub fn eval_bool_query(&mut self, query: String) -> Result<bool, Error> {
        self.engine.eval_bool_query(query, false).map_err(|e| Error::QueryEvaluationError {
            error: e.to_string(),
        })
    }
}

/// Converts a `serde_json::Value` to a `regorus::Value`.
fn convert(data: Value) -> regorus::Value {
    match data {
        Null => {regorus::Value::Null}
        Value::Bool(v) => { regorus::Value::Bool(v) }
        Value::Number(v) => {
            if v.is_i64() {
                regorus::Value::from(v.as_i64().expect("Failed to convert i64 (should not happen)"))
            } else if v.is_u64() {
                regorus::Value::from(v.as_u64().expect("Failed to convert u64 (should not happen)"))
            } else {
                regorus::Value::from(v.as_f64().expect("Failed to convert f64 (should not happen)"))
            }
        }
        Value::String(v) => { regorus::Value::from(v) }
        Value::Array(items) => {
            let mut converted_items = Vec::new();
            for item in items {
                converted_items.push(convert(item));
            }
            regorus::Value::Array(Arc::new(converted_items))
        }
        Value::Object(kv_map) => {
            let mut converted_kv_map = BTreeMap::new();
            for (k, v) in kv_map {
                _ = converted_kv_map.insert(regorus::Value::from(k), convert(v));
            }
            regorus::Value::Object(Arc::new(converted_kv_map))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct Infra {
        servers: Vec<Server>,
        networks: Vec<Network>,
        ports: Vec<Port>,
    }

    #[derive(Serialize, Deserialize)]
    struct Server {
        id: String,
        protocols: Vec<String>,
        ports: Vec<String>,
    }

    #[derive(Serialize, Deserialize)]
    struct Network {
        id: String,
        public: bool,
    }

    #[derive(Serialize, Deserialize)]
    struct Port {
        id: String,
        network: String,
    }

    #[test]
    fn test_add_policy() {
        let mut engine = Engine::new();
        let policy_path = "data/policies/hello_world.rego";
        assert!(engine.add_policy(policy_path).is_ok());
    }

    #[test]
    fn test_eval() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = Engine::new();
        engine.add_policy("data/policies/hello_world.rego")?;

        let results = engine.eval_query("data.test.message".to_owned())?;
        println!("{}", serde_json::to_string_pretty(&results)?);
        // ToDo
        Ok(())
    }

    #[test]
    fn test_add_data() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = Engine::new();
        engine.add_policy("data/policies/infra.rego")?;
        let infra = create_infra();
        engine.add_data(&infra)?;
        let allow = engine.eval_bool_query("data.infra.allow".to_owned())?;
        assert!(allow);
        Ok(())
    }

    #[test]
    fn test_set_input() -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = Engine::new();
        engine.add_policy("data/policies/infra.rego")?;
        let infra = create_infra();
        engine.set_input(&infra)?;
        let results = engine.eval_query("data.infra.allow".to_owned())?;
        println!("{}", serde_json::to_string_pretty(&results)?);
        assert_eq!(results.result.len(), 1);
        // ToDo
        Ok(())
    }

    fn create_infra() -> Infra {
        let server1 = Server {
            id: "app".to_owned(),
            protocols: vec!["https".to_owned(), "ssh".to_owned()],
            ports: vec!["p1".to_owned(), "p2".to_owned(), "p3".to_owned()],
        };
        let server2 = Server {
            id: "db".to_owned(),
            protocols: vec!["mysql".to_owned()],
            ports: vec!["p3".to_owned()],
        };
        let server3 = Server {
            id: "cache".to_owned(),
            protocols: vec!["memcache".to_owned()],
            ports: vec!["p3".to_owned()],
        };
        let server4 = Server {
            id: "ci".to_owned(),
            protocols: vec!["http".to_owned()],
            ports: vec!["p1".to_owned(), "p2".to_owned()],
        };
        let server5 = Server {
            id: "busybox".to_owned(),
            protocols: vec!["telnet".to_owned()],
            ports: vec!["p1".to_owned()],
        };
        let servers = vec![server1, server2, server3, server4, server5];

        let network1 = Network {
            id: "net1".to_owned(),
            public: false,
        };
        let network2 = Network {
            id: "net2".to_owned(),
            public: false,
        };
        let network3 = Network {
            id: "net3".to_owned(),
            public: true,
        };
        let network4 = Network {
            id: "net4".to_owned(),
            public: true,
        };
        let networks = vec![network1, network2, network3, network4];

        let port1 = Port {
            id: "p1".to_owned(),
            network: "net1".to_owned(),
        };
        let port2 = Port {
            id: "p2".to_owned(),
            network: "net3".to_owned(),
        };
        let port3 = Port {
            id: "p3".to_owned(),
            network: "net2".to_owned(),
        };
        let ports = vec![port1, port2, port3];

        Infra {
            servers,
            networks,
            ports,
        }
    }
}
