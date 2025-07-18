# Registry Validation

<kbd>weaver registry check</kbd> ([Usage](usage.md#registry-check))

The validation process for a semantic convention registry involves several steps:
- Loading the semantic convention specifications from a local directory or a git repository.
- Parsing the loaded semantic convention specifications.
- Resolving references and extends clauses within the specifications.
- Checking compliance with specified Rego policies, if provided.


## Custom rules (Rego)

Specific validation rules can be expressed using the Rego policy language (https://www.openpolicyagent.org/docs/policy-language).

Please see [Weaver Forge](https://github.com/open-telemetry/weaver/blob/main/crates/weaver_forge/README.md) for details.
