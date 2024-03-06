// SPDX-License-Identifier: Apache-2.0

//!

use minijinja::{Error, State, Value};

pub fn attributes(_state: &State, v: Value) -> Result<Value, Error> {
    Ok(v)
}