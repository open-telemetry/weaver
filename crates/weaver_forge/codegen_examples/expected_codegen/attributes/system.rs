/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! Describes System CPU attributes
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/attributes/attributes.rs.j2

/// The logical CPU number [0..n-1]
///
/// Examples:
/// - 1
#[cfg(feature = "semconv_experimental")]
pub const SYSTEM_CPU_LOGICAL_NUMBER: crate::attributes::AttributeKey<i64> = crate::attributes::AttributeKey::new("system.cpu.logical_number");

/// The state of the CPU
///
/// Examples:
/// - idle
/// - interrupt
#[cfg(feature = "semconv_experimental")]
pub const SYSTEM_CPU_STATE: crate::attributes::AttributeKey<SystemCpuState> = crate::attributes::AttributeKey::new("system.cpu.state");

/// The state of the CPU
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SystemCpuState {
    #[cfg(feature = "semconv_experimental")] 
    User,
    #[cfg(feature = "semconv_experimental")] 
    System,
    #[cfg(feature = "semconv_experimental")] 
    Nice,
    #[cfg(feature = "semconv_experimental")] 
    Idle,
    #[cfg(feature = "semconv_experimental")] 
    Iowait,
    #[cfg(feature = "semconv_experimental")] 
    Interrupt,
    #[cfg(feature = "semconv_experimental")] 
    Steal,
    /// This variant allows defining a custom entry in the enum.
    _Custom(String),
}

impl SystemCpuState {
    /// Returns the string representation of the [`SystemCpuState`].
    pub fn as_str(&self) -> &str {
        match self {
            #[cfg(feature = "semconv_experimental")] 
            SystemCpuState::User => "user",
            #[cfg(feature = "semconv_experimental")] 
            SystemCpuState::System => "system",
            #[cfg(feature = "semconv_experimental")] 
            SystemCpuState::Nice => "nice",
            #[cfg(feature = "semconv_experimental")] 
            SystemCpuState::Idle => "idle",
            #[cfg(feature = "semconv_experimental")] 
            SystemCpuState::Iowait => "iowait",
            #[cfg(feature = "semconv_experimental")] 
            SystemCpuState::Interrupt => "interrupt",
            #[cfg(feature = "semconv_experimental")] 
            SystemCpuState::Steal => "steal",
            SystemCpuState::_Custom(v) => v.as_str(),
            // Without this default case, the match expression would not
            // contain any variants if all variants are annotated with the
            // 'semconv_experimental' feature and the feature is not enabled.
            #[allow(unreachable_patterns)]
            _ => unreachable!(),
        }
    }
}

impl core::fmt::Display for SystemCpuState {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl crate::attributes::AttributeKey<SystemCpuState> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: &SystemCpuState) -> opentelemetry::KeyValue {
        opentelemetry::KeyValue::new(self.key.clone(), v.to_string())
    }
}

/// The memory state
///
/// Examples:
/// - free
/// - cached
#[cfg(feature = "semconv_experimental")]
pub const SYSTEM_MEMORY_STATE: crate::attributes::AttributeKey<SystemMemoryState> = crate::attributes::AttributeKey::new("system.memory.state");

/// The memory state
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SystemMemoryState {
    #[cfg(feature = "semconv_experimental")] 
    Used,
    #[cfg(feature = "semconv_experimental")] 
    Free,
    #[cfg(feature = "semconv_experimental")] 
    Shared,
    #[cfg(feature = "semconv_experimental")] 
    Buffers,
    #[cfg(feature = "semconv_experimental")] 
    Cached,
    /// This variant allows defining a custom entry in the enum.
    _Custom(String),
}

impl SystemMemoryState {
    /// Returns the string representation of the [`SystemMemoryState`].
    pub fn as_str(&self) -> &str {
        match self {
            #[cfg(feature = "semconv_experimental")] 
            SystemMemoryState::Used => "used",
            #[cfg(feature = "semconv_experimental")] 
            SystemMemoryState::Free => "free",
            #[cfg(feature = "semconv_experimental")] 
            SystemMemoryState::Shared => "shared",
            #[cfg(feature = "semconv_experimental")] 
            SystemMemoryState::Buffers => "buffers",
            #[cfg(feature = "semconv_experimental")] 
            SystemMemoryState::Cached => "cached",
            SystemMemoryState::_Custom(v) => v.as_str(),
            // Without this default case, the match expression would not
            // contain any variants if all variants are annotated with the
            // 'semconv_experimental' feature and the feature is not enabled.
            #[allow(unreachable_patterns)]
            _ => unreachable!(),
        }
    }
}

impl core::fmt::Display for SystemMemoryState {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl crate::attributes::AttributeKey<SystemMemoryState> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: &SystemMemoryState) -> opentelemetry::KeyValue {
        opentelemetry::KeyValue::new(self.key.clone(), v.to_string())
    }
}