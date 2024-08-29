# Multi-Registry Specification Proposal

Status: Work in Progress

## Introduction 

A series of changes are proposed to support multiple semantic convention registries in OpenTelemetry.

Note: This proposal tries to describe the overall changes needed to support the vision but that does not mean that we
can't introduce the changes incrementally. For example, we could start by only supporting attributes and then
progressively add support for metrics, events, spans, etc.

## Use Case Example

The following use case doesn't pretend to be exhaustive but it should give a good idea of the kind of multi-registry
use cases we'd like to support.

[Diagram](TBD)

Actors:
- OTEL: The OpenTelemetry project that publishes the OTEL semantic convention registry. So the community can discover
  and use the signals defined by OTEL.
- WAF Vendor: A vendor of a Web Application Firewall (WAF) that wants to publish a semantic convention registry for
  their product. So their customers can discover and use their signals.
- Cloud Vendor: A cloud provider that wants to publish a semantic convention registry for their services.
- OSS lib author: An author of an OSS library that wants to publish a semantic convention registry for his library.
- Enterprise App: An enterprise that wants to use the concept of semantic convention registry for their internal use.

ToDo
- Actors
- Diagram
- Narrative
- Value Proposition for the different actors

Benefits/Value Proposition per actor:
- OTEL: 
  - Can focus on the core signals and delegate the definition of more specific signals to the community.
  - Can leverage the community to define more specific signals.
  - Can leverage the community to define signals for specific domains.
  - Can leverage the community to define signals for specific vendors.
- WAF Vendor:
  - Reuse common signals defined by OTEL. So overall their customer experiences will be more consistent across
    different vendors.
  - Make it easier for their customers to discover and use their custom signals.
- Cloud Vendor:
- OSS lib author:
- App Developer:

## Semantic Convention Registry Changes

- A semantic convention registry can be defined by anyone. For all the following examples, registry authors can 
  extend or amend the OTEL registry or create their own attributes and groups:
  - A vendor publishes a semconv registry for their products, so their customers can discover and use their signals
  - A community publishes a semconv registry for a specific domain that is too specific to be included in the OTEL registry
  - An individual publishes a semconv registry for their own OSS library or project
  - An enterprise creates internal semconv registries for their internal use
- A semantic convention registry can import one or several semantic conventions from other published registries.
- A new optional section called `imports` will be added to semantic convention file defining groups. 
- The `imports` section is a list of imported semantic conventions with their schema URL and alias.
- Aliases are only visible inside the file where they are defined.
- Aliases must be unique inside the file where they are defined.
- Schema URLs are used to fetch both OTEL schema and self-contained/resolved semantic convention registries. The way a
  resolved registry is linked to an OTEL schema is TBD (could be a new URL pointing to the resolved registry or an
  integration inside the schema file itself). 
- Unused imported registries will be detected by Weaver and reported as warnings.
- A registry can only be imported as a self-contained/resolved semantic convention registry.
- A set of core policies will be enforced by Weaver for any registry OTEL or non-OTEL in order to ensure backward
  compatibility and consistency across registries (list of core policies TBD).
- Any attribute or group of a registry is a referencable entity when the registry is imported. 
- Group references are now supported to support the following use cases  
  - A registry can add new attributes to a group defined in another registry.
  - A registry can override the attributes of a group defined in another registry (e.g. `requirement_level`).
  - A registry can override a subset of group fields defined in another registry (list of fields TBD).
- Overrides defined in a registry are not propagated to the imported semantic conventions.
- Overrides defined in a registry are visible to registries importing the current registry. These attributes and groups
  overrides are re-exported with some transformations by the local registry.
- Group reference can't change the type of the group (similar to attribute reference).
- References to an imported group or attribute are always prefixed with the alias of the imported semantic  convention
  (e.g. `ref: otel:client.address`). The colon is used as a separator between the alias and the group or attribute name.
- References to entities (groups or attributes) defined in the local registry are never prefixed.
- A locally defined group can reference an imported group in its `extends` section.
- A locally defined group can reference an imported attribute in its `attributes` section.

Note: A resolved semantic convention registry is self-contained and does not contain any complex constructs like
`imports`, `ref`, `extends`, etc. Their structure are less subject to change, making them good candidate for
publication, and making them easier to consume.

Wonkiness to remove from the existing semantic convention schema:

- Rename `metric_name` to `name` in the `metric` group for consistency with the other groups.
- Probably more TBD.

Things we should avoid/minimize:

- Name squatting: By relying on local aliases and URL schema, we are not relying on a naming convention approach based
  on company names, etc. This should minimize the risk of name squatting.
- Name inconsistency: By enforcing core policies, we should minimize the risk of name inconsistency across registries.

Alternatives:

- We could make alias optional in the `imports` section. To do so, we would need to rely on Weaver to automatically
  detect entity IDs which are defined both in the local semconv file and the imported registry. When a such conflict is
  detected, Weaver will report an error and asl the user to define an alias for the imported registry. This approach
  could be supported in the future if we see a need for it.

Open Questions:
- Do we allow different versions of the same registry to be imported into different semantic convention files of the
  same registry?
- Is there a relationship to define between the instrumentation scope name and version and the semantic convention
  registry?

## OpenTelemetry Schema Changes

The OpenTelemetry schema file structure must be updated to either include the URL to a self-contained/resolved
semantic convention or to include the resolved registry itself. 

## Weaver Changes

The following changes are proposed to Weaver:

- Weaver must be able to support any operation on any semantic convention registry (check, resolve, generate, search,
  ...).
- The command `weaver registry generate` must allow the generation of the referenced entities that belong to the
  imported semantic convention registries or optionally the generation of all the entities of the imported registries.
- Extend the `--templates` parameter to allow git URL so OTEL templates (or community-based templates) can be reused
  for any registry.
- Extend the `--policies` parameter to allow git URL so OTEL policies (or community-based policies) can be reused for
  any registry.
- Add a step before the resolution process to build a deduplicated list of the imported registries. Download the
  corresponding resolved registries and create a mapping url to resolved registry that will be used during the
  resolution process to resolve the references to the imported registries (and to detect clashes between local and
  imported IDs if aliases are not used).
- More TBD.

Open Questions: 
- Is a resolved registry contain any trace of the imported registries?

## Protocol Changes

No impact on OTLP and OTAP. 

A `schema_url` field is already present at the resource and scope levels. 

Ideally any component of the observability pipeline should be able to fetch the resolved semantic convention registry
just by knowing the schema URL of any resource or instrumentation scope. 

## OpenTelemetry SDKs Changes

TBD

Open Questions:

- Can we enforce the presence of the schema URL in the resource and instrumentation scope?
- How do we convey the schema URL to the SDKs? Could that be part of the codegen done by Weaver?

## Resolved Semantic Convention Registry Format

The following properties are proposed for a resolved semantic convention registry:

- Resolved semantic convention registry must be easy to consume and to publish
  - Accessible via a URL.
  - Self-contained, i.e. a single file.
  - No `ref`, no `extends`, no `imports`, no alias, no other complex constructs.
  - Yaml or JSON format so resolved registries can be easily consumed by any tool.
- Optional lineage section.

The content of a resolved semantic convention registry depends on the:
- The semantic convention files composing the registry to resolve.
- The semantic convention registries imported.
- The configuration specified during the resolution process.
  - Include all the entities of the imported registries
  - Include only the referenced entities of the imported registries.

More specifically, a resolved semantic convention registry contains:
- All the attributes registry specified locally in the semantic convention registry.
- All the groups specified locally in the semantic convention registry.
- All the attributes and groups imported but not re-exported locally are not included in the resolved registry. A
  re-exported entity is an entity that is imported and referenced in the local registry with some overriding.

Open Questions:

- Do we keep track of the imported registries in the resolved registry? If yes, how? Lineage?
- Can we leverage the attribute deduplication mechanism to simplify the merging of imported registries?
- Can we extend the deduplication mechanism to the signals? 
- Materialized resolved registry (what see the jq, template and policy engines) vs Published resolved registry.
  - Materialized Resolved Registry: This is what the jq, template and policy engines see. In this format there are
  no deduplication of declaration. This format is not meant to be published.
  - Published Resolved Registry: In this format, the deduplication of declaration is automatically done by Weaver. 
  This format is meant to be published.

## Priorities

TBD

- [Not Final] Start with attributes