/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! OpenTelemetry Semantic Convention Attributes
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/lib.rs.j2

use opentelemetry::{Key, KeyValue, StringValue};


/// These attributes may be used to describe the client in a connection-based network interaction where there is one side that initiates the connection (the client is the side that initiates the connection). This covers all TCP network interactions since TCP is connection-based and one side initiates the connection (an exception is made for peer-to-peer communication over TCP where the "user-facing" surface of the protocol / API doesn't expose a clear notion of client and server). This also covers UDP network interactions where one side initiates the interaction, e.g. QUIC (HTTP/3) and DNS.
pub mod client;
/// These attributes may be used for any disk related operation.
pub mod disk;
/// This document defines the shared attributes used to report an error.
pub mod error;
/// This document defines the shared attributes used to report a single exception associated with a span or log.
pub mod exception;
/// This document defines semantic convention attributes in the HTTP namespace.
pub mod http;
/// These attributes may be used for any network related operation.
pub mod network;
/// The operating system (OS) on which the process represented by this resource is running.
pub mod os;
/// Attributes reserved for OpenTelemetry
pub mod otel;
/// Attributes used by non-OTLP exporters to represent OpenTelemetry Scope's concepts.
pub mod otel_scope;

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