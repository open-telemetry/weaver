/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! Attributes reserved for OpenTelemetry
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/lib.rs.j2

use opentelemetry::StringValue;
use crate::AttributeKey;


/// Name of the code, either "OK" or "ERROR". MUST NOT be set if the status code is UNSET.
pub const OTEL_STATUS_CODE: AttributeKey<OtelStatusCode> = AttributeKey::new("otel.status_code");


/// Name of the code, either "OK" or "ERROR". MUST NOT be set if the status code is UNSET.
#[non_exhaustive]
pub enum OtelStatusCode {
    /// The operation has been validated by an Application developer or Operator to have completed successfully.
    Ok,
    /// The operation contains an error.
    Error,

}


/// Description of the Status if it has a value, otherwise not set.
pub const OTEL_STATUS_DESCRIPTION: AttributeKey<StringValue> = AttributeKey::new("otel.status_description");


