# Semantic Convention Registry Data Model

> Status: Beta, Coverage target: > 80%

This crate describes the data model for the OpenTelemetry semantic convention registry. It provides serialization and
deserialization support for YAML files adhering to the semantic convention registry. Serde annotation are used for the
serialization and deserialization of the data model making it easy to read and write YAML, JSON, and other formats
supported by the Serde ecosystem.

For more details on the syntax and semantics, see the [semantic convention YAML language](https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/syntax.md)
documentation.

For a formal definition of the allowed syntax, see the [build-tools JSON schema](https://github.com/open-telemetry/build-tools/blob/main/semantic-conventions/semconv.schema.json).

# Design Principles

- Collect as many warnings and errors as possible. Do not stop at the first error; this approach helps the user fix
  multiple issues at once.
- Rely on the Serde ecosystem for serialization and deserialization. This reliance simplifies support for multiple
  formats such as YAML, JSON, etc.
- This crate is foundational for the OpenTelemetry Weaver project. Therefore, it is crucial to keep the API stable and
  user-friendly. Maintaining a test coverage greater than 80% is important. Test as many as possible error cases/paths.