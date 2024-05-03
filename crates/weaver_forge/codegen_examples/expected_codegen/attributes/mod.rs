/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! OpenTelemetry Semantic Convention Attributes
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/attributes/mod.rs.j2

use opentelemetry::{Key, KeyValue, StringValue};


/// Attributes for the `client` namespace.
pub mod client;
/// Attributes for the `error` namespace.
pub mod error;
/// Attributes for the `exception` namespace.
pub mod exception;
/// Attributes for the `http` namespace.
pub mod http;
/// Attributes for the `network` namespace.
pub mod network;
/// Attributes for the `server` namespace.
pub mod server;
/// Attributes for the `system` namespace.
pub mod system;
/// Attributes for the `url` namespace.
pub mod url;

/// A typed attribute key.
pub struct AttributeKey<T> {
    key: Key,
    phantom: std::marker::PhantomData<T>
}

impl <T> AttributeKey<T> {
    /// Returns a new [`AttributeKey`] with the given key.
    pub(crate) const fn new(key: &'static str) -> AttributeKey<T> {
        Self {
            key: Key::from_static_str(key),
            phantom: std::marker::PhantomData
        }
    }

    /// Returns the key of the attribute.
    pub fn key(&self) -> &Key {
        &self.key
    }
}

impl AttributeKey<StringValue> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: StringValue) -> KeyValue {
        KeyValue::new(self.key.clone(), v)
    }
}

impl AttributeKey<i64> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: i64) -> KeyValue {
        KeyValue::new(self.key.clone(), v)
    }
}

impl AttributeKey<f64> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: f64) -> KeyValue {
        KeyValue::new(self.key.clone(), v)
    }
}

impl AttributeKey<bool> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: bool) -> KeyValue {
        KeyValue::new(self.key.clone(), v)
    }
}