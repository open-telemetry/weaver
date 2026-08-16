# Import fixtures for the entity association series

These registries back the tests in `weaver_resolver` for the entity association
work. Each pull request in that series adds the fixtures that its tests need.

| Registry | depends on | imports | expected |
| --- | --- | --- | --- |
| `base` | – | – | defines the `host` entity |
| `middle_reexport` | base | `entities: [host]` | re-exports `host` to its own consumers |
| `top_diamond` | middle_reexport **and** base | metric + entity | one `host`, and no duplicate warnings |
| `legacy_resource` | – | – | a v1 registry with both spellings of a `type: resource` group |
| `top_legacy_import` | legacy_resource | `entities: [browser]` | the import reaches the legacy group |
| `rival_base` | – | – | defines its own unrelated `host` entity |
| `top_rival_import` | base **and** rival_base | `entities: [host]` | two `host` entities, and a reported clash |
| `top_bad_import` | middle_reexport | `entities: [no.such.entity]` | one unmatched-import warning |
| `top_bad_imports_all_types` | middle_reexport | one bad name per signal type | five warnings, and silence for the two names that bind |
| `middle` | base | – | associates with a local entity and with `host` of base; both resolve |
| `middle_bad_assoc` | base | – | an association that nothing defines fails the resolve |
| `base_excluded` | – | – | defines `host` and keeps it private |
| `middle_excluded_assoc` | base_excluded | – | an association to a private entity fails the resolve |
| `middle_assoc_export` | base | – | one metric, associated with `host` of base |
| `top_rebind` | middle_assoc_export | `metrics: [middle.request.count]` | the imported association still names `host` of base |
| `top_private_first` | base_excluded **then** rival_base | – | the association reaches the public `host` of rival |
| `top_public_first` | rival_base **then** base_excluded | – | the same result, from the same two dependencies in the other order |
| `top_ambiguous_assoc` | base **and** rival_base | – | two registries declare `host`, and the association is reported as ambiguous |
| `top_diamond_assoc` | middle_reexport **and** base | – | two paths, one definition, so the association resolves |
| `top_transitive_assoc` | middle_assoc_export | – | the entity that dependency names is out of reach; the resolve fails |
| `base_refined` | – | – | defines `host` and a refinement of it, `host.windows` |
| `middle_refinement_assoc` | base_refined | – | an association names the refinement of a dependency's entity |
| `base_metric` | – | – | defines `host` and a metric a dependent can refine |
| `middle_refined_signal` | base_metric | – | a metric refinement declares its own association |
| `local_shapes` | – | – | every expression shape, on a metric, an event and a span |
| `local_private_assoc` | – | – | a registry associates with an entity it keeps private from dependents |
| `middle_name_clash` | – | – | a metric named `host` does not satisfy an association naming `host` |
| `legacy_by_id` | – | – | naming a legacy `resource` group by its id, rather than its type, fails |

## An association is not an import

`middle` names the `host` entity of `base` in `entity_associations` and imports
nothing. The association resolves, and the resolved reference names `base` as
the registry that holds the definition. The entity itself stays in `base`: there
is one definition, in one place.

`middle_reexport` imports the same entity, so `host` is in its own registry and
the reference carries no provenance.

## A diamond is not a clash

`top_diamond` and `top_rival_import` look alike and mean opposite things.
Deduplication buckets candidates by the registry that declared them, so the
two cases separate on origin.

`top_diamond` reaches one definition in `base` by two paths, so there is
nothing to choose between and the entity appears once. `top_rival_import`
reaches two definitions that merely share an id, so both are imported and the
duplicate is reported. Collapsing those would drop a definition the registry
asked for, and the author is the one who has to resolve it.

An association splits the same way, in `top_diamond_assoc` and
`top_ambiguous_assoc`. The difference is what an association can do about it: a
signal belongs to one entity, so where an import keeps both and reports a
duplicate, an association has no answer at all and the resolve fails. Importing
one of the two, or defining the entity here, settles it.

## A resolved association travels with the signal

`top_rebind` imports the metric of `middle_assoc_export` and defines its own
`host`, which shares a name with the `host` of `base` and nothing else. The
association resolved once, in `middle_assoc_export`, against `base`. Importing
the metric must carry that answer over rather than ask the question again in a
registry where the same name means something else.

## What a name may reach

A leaf names an entity type or the id of an entity refinement, in the one
namespace `extends` gives them. `local_shapes` and `middle_refinement_assoc`
cover both, for an entity of this registry and of a dependency.

It reaches nothing else. A signal of another kind that shares the name is no
entity (`middle_name_clash`), and the id of a legacy `type: resource` group is
no entity type, whatever an `imports` pattern may match (`legacy_by_id`).

Nor does it reach further than one hop. `top_transitive_assoc` depends on a
registry that names `host` without importing it, so `host` is no part of what
that registry offers, exactly as with an `imports` block.

## A private entity is not in the surface

`top_private_first` and `top_public_first` list the same two dependencies in
opposite orders. `base_excluded` keeps its `host` private and `rival_base`
publishes one, so exactly one `host` is reachable and the order the two are
listed in says nothing about which. A lookup that stops at the first dependency
holding the name would answer differently for the two.

## Imports are not transitive

An import matches the resolved surface of the registries in the manifest. A
registry further down the chain is reachable only when the registry in between
re-exports it. `middle_reexport` does this.

This rule holds for every signal type. `top_diamond` therefore reaches `host`
through `middle_reexport` and also through its direct dependency on `base`.
Both paths lead to one definition in `base`, so the result is one entity.
