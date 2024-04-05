# Policy Engine - Proposal

## Goals

The management of the evolution and maintainability of semantic conventions and
application telemetry schemas relies on rules or policies that must be verified
before publishing a new version. The goal of these policies is to ensure the
coherence and quality of the conventions and schemas over the long term. Below
are some examples of policies which are already verified or that could be
verified:
- Not allowing attributes marked as deprecated unless the 'stability' field is
set to 'deprecated'.
- Not allowing attributes with high cardinality.
- Not allowing optional attributes (some environments may want this type of
rule).
- Not allowing name changes.
- Not allowing attributes to be removed from metrics.
- Requiring 'owners' and 'contacts' fields for metrics, spans, and traces.

Currently, some of these policies are checked manually, while others are
checked through code in the Python build tool or in Weaver. Decoupling these
policies from the Weaver code would allow for various improvements, such as
enhancing auditability, easing policy updates and extensions, automating policy
verification in CI/CD pipelines, and supporting both generic OpenTelemetry
policies and company-specific policies.

## Implementation Proposal

The '[Rego](https://www.openpolicyagent.org/docs/latest/policy-language/)' language, made popular by the CNCF project
[Open Policy Agent](https://www.openpolicyagent.org/) (OPA),
appears to be a suitable candidate for expressing these policies in a
declarative language. The entire set of OpenTelemetry policies could be
expressed in a `Rego` file, versioned in the semantic conventions repository. 
Policies specific to a company could be expressed in another versioned `Rego`
file in the company's repository. The Weaver tool could be extended to verify
these policies in various phases (e.g. before or after resolution). The
`weaver check registry` command could be expanded to accept one or more `Rego`
files as parameters, representing the policies to be verified in a specific
context.

The '[regorus](https://github.com/microsoft/regorus)' project by Microsoft could be used to implement this feature
without having a dependency on the OPA toolchain, making weaver easy to use in
any CI/CD pipeline, such as OpenTelemetry or pipelines of any vendor/company.

### Policies on unresolved semantic conventions

The policy verification could operate as follows:
- Read semconv files of the new version
- Read semconv files of the previous version (if exists)
- Apply `rego` policies on these two inputs
- Display detected violations

Example of a policy expressed in `Rego`:
```rego
package otel

# A registry attribute groups containing at least one `ref` attribute is considered invalid.
violations[violation] {
    group := data.groups[_]
    startswith(group.id, "registry.")
    attr := group.attributes[_]
    attr.ref != null
    violation := {
        "violation": "invalid_registry_ref_attribute",
        "group": group.id,
        "attr": attr.ref,
        "severity": "high",
        "category": "registry"
    }
}

# An attribute marked as stable and deprecated is invalid.
violations[violation] {
    group := data.groups[_]
    attr := group.attributes[_]
    attr.stability == "stable"
    attr.deprecated
    violation := {
        "violation": "invalid_attribute_deprecated_stable",
        "group": group.id,
        "attr": attr.id,
        "severity": "high",
        "category": "attribute"
    }
}

# other violations rules here...
```

These policies applied to the following semconv file...
```yaml
groups:
  - id: registry.network
    prefix: network
    type: attribute_group
    brief: >
      These attributes may be used for any network related operation.
    attributes:
      - id: protocol.name.1
        stability: stable
        type: string
        brief: '[OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.'
        note: The value SHOULD be normalized to lowercase.
        examples: ['amqp', 'http', 'mqtt']
        deprecated: true
      - id: protocol.name.2
        stability: stable
        type: string
        brief: '[OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.'
        note: The value SHOULD be normalized to lowercase.
        examples: ['amqp', 'http', 'mqtt']
      - ref: protocol.port
        deprecated: true
```

... will generate the following violations.

```json
[
  {
    "attr": "protocol.name.1",
    "category": "attribute",
    "group": "registry.network",
    "severity": "high",
    "violation": "invalid_attribute_deprecated_stable"
  },
  {
    "attr": "protocol.port",
    "category": "registry",
    "group": "registry.network",
    "severity": "high",
    "violation": "invalid_registry_ref_attribute"
  }
]
```

`severity` and `category` fields are just an attempt to categorize the
violations and could be removed if not needed.


### Policies on resolved semantic conventions

The policy verification could operate as follows:
- Read and Resolve semconv files of the new version
- Read and Resolve semconv files of the previous version (if exists)
- Apply `rego` policies on these two resolved schemas
- Display detected violations