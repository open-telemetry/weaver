# Multi-Registry - Draft Proposal

Status: Work in Progress

## Introduction 

A series of changes are proposed to support multiple semantic convention registries in OpenTelemetry.

> Note: This proposal aims to describe the overall changes needed to realize this vision. However,
> this does not mean that we cannot introduce the changes incrementally. For example, we could start
> by supporting attributes only and then progressively add support for metrics, events, spans, etc.

## Use Case Example

The following use case is not intended to be exhaustive, but it should provide a good idea of the
types of multi-registry scenarios we aim to support.

The diagram below illustrates a small but concrete example of how multiple semantic convention
registries could be used together.

![Multi-Registry Use Case](images/multi_registry_use_case.svg)

The color-coding within the signal descriptions indicates the provenance of the corresponding definition.

**Actors and Their Benefits** 

1. **OpenTelemetry:**
  - **Value Proposition:**
    - OTEL can focus on defining core signals while delegating the creation of more specific signals
      to the community at scale.
    - OTEL establishes the foundation for developing new uses and tools in the observability ecosystem. 

2. **Vendor:**
  - **Value Proposition:**
    - By publishing their own semantic convention registry, vendors make it easier for their customers
      to discover and effectively use custom signals specific to their products.
    - Vendors can reuse common signals defined by OTEL, ensuring consistency in customer experiences. This
      strategy enhances the interoperability of their products within the larger observability ecosystem.

3. **OSS Library Author:**
  - **Value Proposition:**
    - The OSS library author can reuse OTEL-defined attributes and signals, integrating them with custom
      signals tailored to their library.
    - Publishing a semantic convention registry for the library simplifies the integration process for
      developers, making it easier to adopt and use the library in a consistent and standardized way.
    - This approach also increases the attractiveness, visibility and usability of the library within
      the community.

4. **Enterprise Application:**
  - **Value Proposition:**
    - Enterprises can leverage the concept of semantic convention registries to import external registries
      and simplify their observability integration.
    - By creating internal registries, enterprises can define custom signals that align with their specific
      needs and share these across teams and products, fostering internal consistency.
    - This capability enhances collaboration and streamlines the observability practices within the organization.

By supporting these actors and their specific use cases, the multi-registry approach enables a flexible,
community-driven model for defining and using semantic conventions across diverse domains and applications.

## Design Principles

- **Independent Ownership**: Any individual or organization should be able to create and maintain a semantic convention
  registry independently, without requiring active coordination with the OTEL project.
- **Registry Accessibility**: Semantic convention registries can be either private or public, allowing flexibility
  based on the needs of the owner.
- **Community Support Tools**: The OTEL project will provide and maintain tools to assist the community in creating, 
  validating, resolving, and publishing semantic convention registries (i.e. Weaver tool).
- **Core Policy Enforcement**: The OTEL project will establish and enforce a set of core policies (e.g. backward
  compatibility policies) that all semantic convention registries must adhere to, ensuring consistency and reliability.
- **Cross-Registry References**: References between different semantic convention registries should be supported,
  facilitating interoperability and integration across various registries.
- **Circular Reference Handling**: Circular references between semantic convention registries must be detected,
  reported, and rejected to prevent conflicts and maintain the integrity of the system.

## Semantic Convention Registry Changes

- A semantic convention registry can be defined by anyone, without requiring any active coordination with OTEL.
  For all the following examples, registry authors can extend or amend the OTEL registry or create their own
  attributes and groups (non exhaustive list):
  - A vendor publishes a semantic convention registry for their products, allowing their customers to discover
    and use their signals.
  - A community publishes a semantic convention registry for a specific domain that is too specialized to be
    included in the OTEL registry.
  - An individual publishes a semantic convention registry for their own OSS library or project.
  - An enterprise creates internal semantic convention registries for internal use.
- A semantic convention registry can import one or several semantic conventions from other published registries.
- A new optional section called `imports` will be added to the semantic convention file defining groups.
- The `imports` section is a list of imported semantic conventions with their schema URLs and aliases.
- Aliases are only visible within the file where they are defined.
- Aliases must be unique within the file where they are defined.
- Schema URLs are used to fetch both OTEL schema and self-contained/resolved semantic convention registries.
  The way a resolved registry is linked to an OTEL schema is TBD (it could be a new URL pointing to the resolved
  registry or an integration within the schema file itself).
- Unused imported registries will be detected by Weaver and reported as warnings.
- A registry can only be imported as a self-contained/resolved semantic convention registry.
- A set of core policies will be enforced by Weaver for any registry, OTEL or non-OTEL, to ensure backward
  compatibility and consistency across registries (list of core policies TBD).
- Any attribute or group in a registry is a referencable entity when the registry is imported.
- Group references are now supported to address the following use cases:
  - A registry can add new attributes to a group defined in another registry.
  -	A registry can override the attributes of a group defined in another registry (e.g., `requirement_level`).
  - A registry can only override a subset of group fields defined in another registry (list of fields TBD).
- Overrides defined in a registry are not propagated to the imported semantic conventions.
- Overrides defined in a registry are visible to registries importing the current registry. These attribute
  and group overrides are re-exported with some transformations by the local registry.
- A group reference cannot change the type of the imported group (similar to attribute references).
- References to an imported group or attribute are always prefixed with the alias of the imported semantic
  convention (e.g., `ref: otel:client.address`). The colon is used as a separator between the alias and the
  group or attribute name.
- References to entities (groups or attributes) defined in the local registry are never prefixed.
- A locally defined group can reference an imported group in its `extends` section.
- A locally defined group can reference an imported attribute in its `attributes` section.

> Note: The JSON Schema for the semantic convention registry must be updated to reflect these changes.

> Note: A resolved semantic convention registry is self-contained and does not include any complex constructs
> like `imports`, `ref`, `extends`, etc. Their **structure is less subject to change**, making them good
> candidates for publication and easier to consume.

Wonkiness to remove from the existing semantic convention schema:

- Rename `metric_name` to `name` in the `metric` group for consistency with other groups.
- Probably more TBD.

Things to avoid/minimize:

- **Name squatting**: By relying on local aliases and URL schema, we reduce the risk of name squatting, as
  the naming convention is not based on company names that are not necessarily unique and are complex to control.
- **Name inconsistency**: By enforcing core policies, we minimize the risk of name inconsistency across registries.

Alternatives:

- We could make aliases optional in the imports section. To do so, we would need to rely on Weaver to automatically
  detect entity IDs that are defined both in the local semantic convention file and the imported registry. When such
  a conflict is detected, Weaver will report an error and ask the user to define an alias for the imported registry.
  This approach could be supported in the future if the need arises.

Open Questions:
- Do we allow different versions of the same registry to be imported into different semantic convention files of
  the same registry?
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