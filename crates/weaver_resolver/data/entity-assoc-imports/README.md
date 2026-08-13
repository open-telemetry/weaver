# Import fixtures for the entity association series

These registries back the tests in `weaver_resolver` for the entity association
work. Each pull request in that series adds the fixtures that its tests need.

| Registry | depends on | imports | expected |
| --- | --- | --- | --- |
| `base` | – | – | defines the `host` entity |
| `middle_reexport` | base | `entities: [host]` | re-exports `host` to its own consumers |
| `top_diamond` | middle_reexport **and** base | metric + entity | one `host`, and no duplicate warnings |
| `legacy_resource` | – | – | a v1 registry with a `type: resource` group |
| `top_legacy_import` | legacy_resource | `entities: [browser]` | the import reaches the legacy group |

## Imports are not transitive

An import matches the resolved surface of the registries in the manifest. A
registry further down the chain is reachable only when the registry in between
re-exports it. `middle_reexport` does this.

This rule holds for every signal type. `top_diamond` therefore reaches `host`
through `middle_reexport` and also through its direct dependency on `base`.
Both paths lead to one definition in `base`, so the result is one entity.
