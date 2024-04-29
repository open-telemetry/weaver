/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! Attributes used by non-OTLP exporters to represent OpenTelemetry Scope's concepts.
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/lib.rs.j2

use opentelemetry::StringValue;
use crate::AttributeKey;


/// The name of the instrumentation scope - (`InstrumentationScope.Name` in OTLP).
pub const OTEL_SCOPE_NAME: AttributeKey<StringValue> = AttributeKey::new("otel.scope.name");



/// The version of the instrumentation scope - (`InstrumentationScope.Version` in OTLP).
pub const OTEL_SCOPE_VERSION: AttributeKey<StringValue> = AttributeKey::new("otel.scope.version");


