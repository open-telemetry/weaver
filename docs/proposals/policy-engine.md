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
these policies during schema resolution. The `weaver check registry` command
could be expanded to accept one or more `Rego` files as parameters,
representing the policies to be verified in a specific context.

The '[regorus](https://github.com/microsoft/regorus)' project by Microsoft could be used to implement this feature
without having a dependency on the OPA toolchain.

The policy verification could operate as follows:
- Resolution of the new version of the semconv registry (or the app telemetry
schema)
- Resolution of the previous version of the semconv registry (or the app
telemetry schema)
- Application of policies on these two resolved schemas
- Display of errors if the policies are not adhered to