# Weaver CodeGen Test

This crate is used to test the generation of an unofficial Rust OpenTelemetry Client API derived from a semantic
convention registry. This crate is not intended to be published. It is used solely for testing and validation purposes.

The generated Rust API client exposes a type-safe API (i.e., one that cannot be misused) that adheres to the signal
specification defined in the semantic convention registry located in the semconv_registry directory.