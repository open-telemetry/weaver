// SPDX-License-Identifier: Apache-2.0

//! Custom Tera functions

use std::collections::HashMap;
use std::sync::Arc;

use tera::Result;
use tera::{Function, Value};

use crate::config::DynamicGlobalConfig;

#[derive(Debug)]
pub struct FunctionConfig {
    config: Arc<DynamicGlobalConfig>,
}

impl FunctionConfig {
    pub fn new(config: Arc<DynamicGlobalConfig>) -> Self {
        FunctionConfig { config }
    }
}

impl Function for FunctionConfig {
    fn call(&self, args: &HashMap<String, Value>) -> Result<Value> {
        if let Some(file_name) = args.get("file_name") {
            self.config.set(
                file_name
                    .as_str()
                    .ok_or_else(|| tera::Error::msg("file_name must be a string"))?,
            );
        }
        Ok(Value::Null)
    }

    fn is_safe(&self) -> bool {
        false
    }
}
