/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! OpenTelemetry Semantic Convention Metrics
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/metrics/mod.rs.j2


/// Metrics for the `http` namespace.
pub mod http;

/// A trait implemented by histogram providers (e.g. `Meter`).
pub trait HistogramProvider<T> {
    /// Creates a new histogram with the given name, description, and unit.
    fn create_histogram(&self, name: &'static str, description: &'static str, unit: &'static str) -> opentelemetry::metrics::Histogram<T>;
}

/// This implementation specifies that a Meter is able to create u64 histograms.
impl HistogramProvider<u64> for opentelemetry::metrics::Meter {
    /// Creates a new u64 histogram with the given name, description, and unit.
    fn create_histogram(&self, name: &'static str, description: &'static str, unit: &'static str) -> opentelemetry::metrics::Histogram<u64> {
        self.u64_histogram(name)
            .with_description(description)
            .with_unit(opentelemetry::metrics::Unit::new(unit))
            .init()
    }
}

/// This implementation specifies that a Meter is able to create u64 histograms.
impl HistogramProvider<f64> for opentelemetry::metrics::Meter {
    /// Creates a new f64 histogram with the given name, description, and unit.
    fn create_histogram(&self, name: &'static str, description: &'static str, unit: &'static str) -> opentelemetry::metrics::Histogram<f64> {
        self.f64_histogram(name)
            .with_description(description)
            .with_unit(opentelemetry::metrics::Unit::new(unit))
            .init()
    }
}

/// A trait implemented by up-down-counter providers (e.g. `Meter`).
pub trait UpDownCounterProvider<T> {
    /// Creates a new up-down-counter with the given name, description, and unit.
    fn create_up_down_counter(&self, name: &'static str, description: &'static str, unit: &'static str) -> opentelemetry::metrics::UpDownCounter<T>;
}

/// This implementation specifies that a Meter is able to create i64 up-down-counters.
impl UpDownCounterProvider<i64> for opentelemetry::metrics::Meter {
    /// Creates a new i64 up-down-counter with the given name, description, and unit.
    fn create_up_down_counter(&self, name: &'static str, description: &'static str, unit: &'static str) -> opentelemetry::metrics::UpDownCounter<i64> {
        self.i64_up_down_counter(name)
            .with_description(description)
            .with_unit(opentelemetry::metrics::Unit::new(unit))
            .init()
    }
}

/// This implementation specifies that a Meter is able to create f64 up-down-counters.
impl UpDownCounterProvider<f64> for opentelemetry::metrics::Meter {
    /// Creates a new f64 up-down-counter with the given name, description, and unit.
    fn create_up_down_counter(&self, name: &'static str, description: &'static str, unit: &'static str) -> opentelemetry::metrics::UpDownCounter<f64> {
        self.f64_up_down_counter(name)
            .with_description(description)
            .with_unit(opentelemetry::metrics::Unit::new(unit))
            .init()
    }
}