# Schema Changes Data Model

## Introduction

Weaver can be used to compare two versions of a semantic convention registry
and generate a diff report. This document describes the data model used to
represent the differences between two versions of a semantic convention
registry, as well as the diffing process. This diff report can be used to:

- Understand the changes between two versions of a semantic convention registry.
- Update the OpenTelemetry Schema file, section versions, with the new version.
- Generate a migration guide for users of the semantic convention registry.
- Generate a SQL DDL script to update a database schema.
- And more.

> Note: The current implementation of the diffing process focuses on the top-level
items attributes, metrics, events, spans, resources and does not compare the fields
of those top-level items.

## Data Model

The schema changes data model is composed of the following components:

- `head`: The registry manifest of the most recent version of the semantic
  convention registry used in the diffing process.
- `baseline`: The registry manifest of the oldest version of the semantic
  convention registry used in the diffing process.
- `changes`: A dictionary of changes between the head and baseline registries.
  The dictionary is composed of the following keys (when applicable):
  - `attributes`: A list of changes to attributes.
  - `metrics`: A list of changes to metrics.
  - `events`: A list of changes to events.
  - `spans`: A list of changes to spans.
  - `resources`: A list of changes to resources.

Each change in the changes dictionary for any key is represented as a list of
schema changes, represented by one of the following types:

- `added`: A new schema item (e.g., attribute, metric, etc.) was added in the
  head registry. The name of the new item is stored in the name attribute.
- `renamed_to_new`: One or more schema items in the baseline registry were
  renamed to the same new name in the head registry. The old names of the
  items are stored in the old_names attribute, and the new name is stored in
  the current_name attribute.
- `renamed_to_existing`: One or more schema items in the baseline registry were
  renamed to an existing item in the head registry. The old names of the items
  are stored in the old_names attribute, and the existing item name is stored
  in the current_name attribute.
- `deprecated`: An item in the baseline registry was deprecated in the head
  registry. The name of the deprecated item is stored in the name attribute,
  and the deprecation note is stored in the note attribute.
- `removed`: An item in the baseline registry was removed in the head
  registry. The name of the removed item is stored in the name attribute.

> Note: Although the removed schema change type is a valid output of the diffing
process, it should never be present in the diff report between two versions of
well-formed semantic convention registries. The policy for semantic convention
registries is to deprecate items instead of removing them.

Example Schema Diff in YAML

```yaml
head:
  semconv_version: v1.27.0
baseline:
  semconv_version: v1.26.0
changes:
  attributes:
    - type: deprecated
      name: http.server_name
      note: deprecated
    - type: added
      name: user.email
    - ...
  events:
    - type: added
      name: exception
    - ...
  metrics:
    - type: added
      name: go.goroutine.count
    - type: deprecated
      name: db.client.connections.max
      note: Deprecated
    - ...
```

## Diffing Process

The following rules are applied during the diffing process to generate the schema
changes for attributes:

1. Deprecations:
  - If an attribute in the latest schema is now marked as deprecated, it is
    classified into the following cases:
    - Renamed to new: Attributes in the deprecated metadata pointing to a new
      attribute are marked as renamed_to_new.
    - Renamed to existing: Attributes in the deprecated metadata pointing to an
      already existing attribute are marked as renamed_to_existing.
    - Deprecated without replacement: The attribute is marked as deprecated with
      no replacement.
1. Additions:
  - If an attribute exists in the latest schema but not in the baseline, it is
    classified as a new attribute.
  - However, if this new attribute is referenced in the deprecated metadata of an
    old attribute, it is considered a renamed attribute.
1. Removals:
  - Attributes present in the baseline but missing in the latest schema are marked
    as removed. This should not happen if registry evolution processes are followed.

The diffing process for the signals (metrics, events, spans, resources) is similar
to the attributes comparison.

## Future Evolutions

The current implementation of the diffing process focuses on the top-level schema
items (attributes, metrics, events, spans, resources). Future evolutions of the
diffing process could generate a more detailed diff report by comparing the fields
of those top-level schema items.