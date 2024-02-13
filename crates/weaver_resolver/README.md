# Telemetry Schema Resolution Process

Status: **Work-In-Progress**

This crate describes the resolution process for the OpenTelemetry telemetry
schema. The resolution process takes a telemetry schema and/or a semantic
convention registry and produces a resolved telemetry schema. The resolved
telemetry schema is a self-contained and consistent schema that can be used to
validate telemetry data, generate code, and perform other tasks.

Important Note: Currently, 2 versions of the resolution process are present in
the `weaver_resolver` crate. The first version of the resolution process is
incomplete and still used by the `weaver` CLI. A second version is under active
development and is expected to replace the first version in the near future.