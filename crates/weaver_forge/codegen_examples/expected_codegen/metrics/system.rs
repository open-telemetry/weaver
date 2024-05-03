/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/metrics/metrics.rs.j2

use crate::metrics::{CounterProvider, GaugeProvider, HistogramProvider, UpDownCounterProvider};

/// Seconds each logical CPU spent on each mode
#[cfg(feature = "semconv_experimental")]
pub fn create_system_cpu_time<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Counter<T>
    where opentelemetry::metrics::Meter: CounterProvider<T> {
    meter.create_counter("system.cpu.time", "Seconds each logical CPU spent on each mode", "s")
}

/// Metric: system.cpu.time
/// Brief: Seconds each logical CPU spent on each mode
/// Unit: s
#[derive(Debug)]
pub struct SystemCpuTime<T>(opentelemetry::metrics::Counter<T>);




#[derive(Debug, Clone, Default)]
pub struct SystemCpuTimeOptAttributes {
    
    /// The logical CPU number [0..n-1]
    ///
    /// Examples:
    /// - 1
    pub system_cpu_logical_number: Option<i64>,
    
    /// The CPU state for this data point. A system's CPU SHOULD be characterized *either* by data points with no `state` labels, *or only* data points with `state` labels.
    ///
    /// Examples:
    /// - idle
    /// - interrupt
    pub system_cpu_state: Option<crate::attributes::system::SystemCpuState>,
}


impl <T> SystemCpuTime<T> {
    pub fn new(meter: &opentelemetry::metrics::Meter) -> Self
        where opentelemetry::metrics::Meter: CounterProvider<T>{
        Self(meter.create_counter("system.cpu.time", "Seconds each logical CPU spent on each mode", "s"))
    }

    /// Adds an additional value to the counter.
    pub fn add(
        &self,
        value: T,
        
        optional_attributes: Option<&SystemCpuTimeOptAttributes>,
    ) {
        let mut attributes = vec![
        ];

        if let Some(value) = &optional_attributes {
            #[cfg(feature = "semconv_experimental")]
            if let Some(system_cpu_logical_number) = value.system_cpu_logical_number {
                attributes.push(crate::attributes::system::SYSTEM_CPU_LOGICAL_NUMBER.value(system_cpu_logical_number));
            }
            #[cfg(feature = "semconv_experimental")]
            if let Some(system_cpu_state) = &value.system_cpu_state {
                attributes.push(crate::attributes::system::SYSTEM_CPU_STATE.value(system_cpu_state));
            }
        }
        self.0.add(value, &attributes)
    }
}

/// Difference in system.cpu.time since the last measurement, divided by the elapsed time and number of logical CPUs
#[cfg(feature = "semconv_experimental")]
pub fn create_system_cpu_utilization<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::Gauge<T>
    where opentelemetry::metrics::Meter: GaugeProvider<T> {
    meter.create_gauge("system.cpu.utilization", "Difference in system.cpu.time since the last measurement, divided by the elapsed time and number of logical CPUs", "1")
}

/// Metric: system.cpu.utilization
/// Brief: Difference in system.cpu.time since the last measurement, divided by the elapsed time and number of logical CPUs
/// Unit: 1
#[derive(Debug)]
pub struct SystemCpuUtilization<T>(opentelemetry::metrics::Gauge<T>);




#[derive(Debug, Clone, Default)]
pub struct SystemCpuUtilizationOptAttributes {
    
    /// The logical CPU number [0..n-1]
    ///
    /// Examples:
    /// - 1
    pub system_cpu_logical_number: Option<i64>,
    
    /// The CPU state for this data point. A system's CPU SHOULD be characterized *either* by data points with no `state` labels, *or only* data points with `state` labels.
    ///
    /// Examples:
    /// - idle
    /// - interrupt
    pub system_cpu_state: Option<crate::attributes::system::SystemCpuState>,
}


impl <T> SystemCpuUtilization<T> {
    pub fn new(meter: &opentelemetry::metrics::Meter) -> Self
        where opentelemetry::metrics::Meter: GaugeProvider<T>{
        Self(meter.create_gauge("system.cpu.utilization", "Difference in system.cpu.time since the last measurement, divided by the elapsed time and number of logical CPUs", "1"))
    }

    /// Records an additional value to the gauge.
    pub fn record(
        &self,
        value: T,
        
        optional_attributes: Option<&SystemCpuUtilizationOptAttributes>,
    ) {
        let mut attributes = vec![
        ];

        if let Some(value) = &optional_attributes {
            #[cfg(feature = "semconv_experimental")]
            if let Some(system_cpu_logical_number) = value.system_cpu_logical_number {
                attributes.push(crate::attributes::system::SYSTEM_CPU_LOGICAL_NUMBER.value(system_cpu_logical_number));
            }
            #[cfg(feature = "semconv_experimental")]
            if let Some(system_cpu_state) = &value.system_cpu_state {
                attributes.push(crate::attributes::system::SYSTEM_CPU_STATE.value(system_cpu_state));
            }
        }
        self.0.record(value, &attributes)
    }
}

/// Reports memory in use by state.
#[cfg(feature = "semconv_experimental")]
pub fn create_system_memory_usage<T>(meter: &opentelemetry::metrics::Meter) -> opentelemetry::metrics::UpDownCounter<T>
    where opentelemetry::metrics::Meter: UpDownCounterProvider<T> {
    meter.create_up_down_counter("system.memory.usage", "Reports memory in use by state.", "By")
}

/// Metric: system.memory.usage
/// Brief: Reports memory in use by state.
/// Unit: By
#[derive(Debug)]
pub struct SystemMemoryUsage<T>(opentelemetry::metrics::UpDownCounter<T>);




#[derive(Debug, Clone, Default)]
pub struct SystemMemoryUsageOptAttributes {
    
    /// The memory state
    ///
    /// Examples:
    /// - free
    /// - cached
    pub system_memory_state: Option<crate::attributes::system::SystemMemoryState>,
}


impl <T> SystemMemoryUsage<T> {
    pub fn new(meter: &opentelemetry::metrics::Meter) -> Self
        where opentelemetry::metrics::Meter: UpDownCounterProvider<T>{
        Self(meter.create_up_down_counter("system.memory.usage", "Reports memory in use by state.", "By"))
    }

    /// Adds an additional value to the up-down-counter.
    pub fn add(
        &self,
        value: T,
        
        optional_attributes: Option<&SystemMemoryUsageOptAttributes>,
    ) {
        let mut attributes = vec![
        ];

        if let Some(value) = &optional_attributes {
            #[cfg(feature = "semconv_experimental")]
            if let Some(system_memory_state) = &value.system_memory_state {
                attributes.push(crate::attributes::system::SYSTEM_MEMORY_STATE.value(system_memory_state));
            }
        }
        self.0.add(value, &attributes)
    }
}