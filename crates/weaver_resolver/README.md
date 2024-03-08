# Telemetry Schema Resolution Process

Resolution Process Status:
- Semantic Convention Registry: **Fully Implemented**, **Partially Tested**
- Application Telemetry Schema: **Partially Implemented**

This crate describes the resolution process for the OpenTelemetry telemetry
schema. The resolution process takes a telemetry schema and/or a semantic
convention registry and produces a resolved telemetry schema. The resolved
telemetry schema is a self-contained and consistent schema that can be used to
validate telemetry data, generate code, and perform other tasks.

Important Note: Currently, 2 versions of the resolution process are present in
the `weaver_resolver` crate. The first version of the resolution process is
incomplete and still used by the `weaver` CLI. A second version is under active
development and is expected to replace the first version in the near future.

## Semantic Conventions - Parsing, Resolution and Validation

The parsing is implemented using the serde library. A collection of
serde-annotated Rust structures is used to represent semantic convention
entities. Optional fields are represented as `Option` types.

The attribute macro `#[serde(deny_unknown_fields)]` is used to ensure that
unknown fields are not allowed in the semantic convention entities. This,
combined with the distinction between optional and required fields in the
entities, ensures that the semantic conventions are validated in terms of
structure during the parsing process.

The resolution process for semantic conventions is a multistep process that
involves the following steps:
- Load all semantic conventions from the registry
- Resolve iteratively all semantic conventions. This involves the maintenance
  of an unresolved semantic convention list and a resolved semantic convention
  list. The resolution process involves the following steps:
  - Resolve iteratively all attributes `ref` until no more resolvable `ref` are
    found.
  - Resolve iteratively all `extends` parent/child clauses until no more
    resolvable `extends` are found. The extended entity inherits prefix,
    attributes, and constraints from the parent entity.
- Apply constraints `any_of` and `include`.
- Validate the resolved semantic conventions
  - No more unresolved `ref` or `extends` clauses. The unresolved list should
    be empty.
  - All constraints satisfied.

