/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! The operating system (OS) on which the process represented by this resource is running.
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/lib.rs.j2

use opentelemetry::StringValue;
use crate::AttributeKey;


/// The operating system type.
#[cfg(feature = "semconv_experimental")]
pub const OS_TYPE: AttributeKey<OsType> = AttributeKey::new("os.type");


/// The operating system type.
#[non_exhaustive]
pub enum OsType {
    /// Microsoft Windows
    #[cfg(feature = "semconv_experimental")] 
    Windows,
    /// Linux
    #[cfg(feature = "semconv_experimental")] 
    Linux,
    /// Apple Darwin
    #[cfg(feature = "semconv_experimental")] 
    Darwin,
    /// FreeBSD
    #[cfg(feature = "semconv_experimental")] 
    Freebsd,
    /// NetBSD
    #[cfg(feature = "semconv_experimental")] 
    Netbsd,
    /// OpenBSD
    #[cfg(feature = "semconv_experimental")] 
    Openbsd,
    /// DragonFly BSD
    #[cfg(feature = "semconv_experimental")] 
    Dragonflybsd,
    /// HP-UX (Hewlett Packard Unix)
    #[cfg(feature = "semconv_experimental")] 
    Hpux,
    /// AIX (Advanced Interactive eXecutive)
    #[cfg(feature = "semconv_experimental")] 
    Aix,
    /// SunOS, Oracle Solaris
    #[cfg(feature = "semconv_experimental")] 
    Solaris,
    /// IBM z/OS
    #[cfg(feature = "semconv_experimental")] 
    ZOs,

}


/// Human readable (not intended to be parsed) OS version information, like e.g. reported by `ver` or `lsb_release -a` commands.
#[cfg(feature = "semconv_experimental")]
pub const OS_DESCRIPTION: AttributeKey<StringValue> = AttributeKey::new("os.description");



/// Human readable operating system name.
#[cfg(feature = "semconv_experimental")]
pub const OS_NAME: AttributeKey<StringValue> = AttributeKey::new("os.name");



/// The version string of the operating system as defined in [Version Attributes](/docs/resource/README.md#version-attributes).
#[cfg(feature = "semconv_experimental")]
pub const OS_VERSION: AttributeKey<StringValue> = AttributeKey::new("os.version");



/// Unique identifier for a particular build or compilation of the operating system.
#[cfg(feature = "semconv_experimental")]
pub const OS_BUILD_ID: AttributeKey<StringValue> = AttributeKey::new("os.build_id");


