/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! These attributes may be used for any disk related operation.
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/lib.rs.j2

use opentelemetry::StringValue;
use crate::AttributeKey;


/// The disk IO operation direction.
#[cfg(feature = "semconv_experimental")]
pub const DISK_IO_DIRECTION: AttributeKey<DiskIoDirection> = AttributeKey::new("disk.io.direction");


/// The disk IO operation direction.
#[non_exhaustive]
pub enum DiskIoDirection {

    #[cfg(feature = "semconv_experimental")] 
    Read,

    #[cfg(feature = "semconv_experimental")] 
    Write,

}

