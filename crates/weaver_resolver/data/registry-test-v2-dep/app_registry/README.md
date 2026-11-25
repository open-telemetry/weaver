# App Registry Fixture (`registry-test-v2-dep/app_registry`)

This fixture verifies resolution of an application registry that consumes both:
- `https://consumer.example.com/schemas/1.0.0` (definition registry)
- `https://server.example.com/schemas/1.0.0` (published resolved V2 registry)

Per the V2 specification, references to attributes defined in other registries (`ref: server.address`) require a declared direct dependency on the defining registry rather than relying on non-exported transitive definitions.
