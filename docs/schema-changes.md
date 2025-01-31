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
telemetry object attributes, metrics, events, spans, resources and does not compare
the fields of those top-level items.

## Data Model

The schema changes data model is composed of the following components:

- `head`: The registry manifest of the most recent version of the semantic
  convention registry used in the diffing process.
- `baseline`: The registry manifest of the oldest version of the semantic
  convention registry used in the diffing process.
- `changes`: A dictionary of changes between the head and baseline registries.
  The dictionary is composed of the following keys (when applicable):
  - `registry_attributes`: A list of changes to registry attributes.
  - `metrics`: A list of changes to metrics.
  - `events`: A list of changes to events.
  - `spans`: A list of changes to spans.
  - `resources`: A list of changes to resources.

Each change in the changes dictionary for any key is represented as a list of
schema changes, represented by one of the following change types:

- `added`: A top-level telemetry object (e.g., attribute, metric, etc.) was added to the head registry. The new item’s
  name is stored in the name attribute.
- `renamed`: A top-level telemetry object from the baseline registry was renamed in the head registry.
- `updated`: One or more fields in a top-level telemetry object have been updated in the head registry.
- `obsoleted`: A top-level telemetry object that is now discontinued without a valid replacement in the head registry.
- `uncategorized`: A placeholder for complex or unclear schema changes that do not fit into existing types. This type
  serves as a fallback when no specific category applies, with the expectation that some of these changes will be
  reclassified into more precise schema types in the future.
- `removed`: A top-level telemetry object from the baseline registry was removed in the head registry.

> Note: Although the removed schema change type is a valid output of the diffing
process, it should never be present in the diff report between two versions of
well-formed semantic convention registries. The policy for semantic convention
registries is to deprecate items instead of removing them.

Some of these schema change types are based on the new `deprecated` structured format present in the semantic
conventions. This field is now supporting one of the three variants:

Variant 1
```yaml
brief: <text explaining the reason of the renaming>
deprecated:
  reason: renamed
  renamed_to: <name of the telemetry object>
```

Variant 2
```yaml
brief: <text explaining the reason of the obsolescence>
deprecated:
  reason: obsoleted
```

Variant 3
```yaml
brief: <text explaining the reason of this complex deprecation>
deprecated:
  reason: uncategorized
```

Example Schema Diff in YAML

```yaml
head:
  semconv_version: v1.27.0
baseline:
  semconv_version: v1.26.0
changes:
  registry_attributes:
    - name: http.server_name      # attribute name
      type: obsoleted             # change type
      note: This attribute is deprecated.
    - name: user.email            # attribute name
      type: added                 # change type
    - name: http_target
      type: renamed
      new_name: http.target
      note: Renamed to http.target
    - ...
  events:
    - name: exception
      type: added
    - ...
  metrics:
    - name: go.goroutine.count
      type: added
    - name: db.client.connections.max
      type: obsoleted
      note: Deprecated
    - ...
```

## Diffing Process

The following rules are applied during the diffing process to generate the schema
changes for attributes:

1. Deprecations:
  - A deprecated attribute can result in a `renamed`, `obsoleted`, or `uncategorized` schema change event, depending
    on the specified action.
  - If an attribute in the latest schema is now marked as deprecated and was not before, the schema change type is
    directly derived from the `deprecated.reason` field.
1. Additions:
  - If an attribute exists in the latest schema but not in the baseline, it is classified as a new attribute (`added`).
1. Removals:
  - Attributes present in the baseline but missing in the latest schema are marked as removed (removed).
  - This should not occur if registry evolution processes are properly followed.

The diffing process for the signals (metrics, events, spans, resources) is similar
to the attributes comparison.

> Note: The change type `updated` is not currently implemented in the diffing process.

## Future Evolutions

The current implementation of the diffing process focuses on the top-level schema
items (attributes, metrics, events, spans, resources). Future evolutions of the
diffing process could generate a more detailed diff report by comparing the fields
of those top-level schema items.

This [document](schema-changes-explorations-for-future-evolutions.md) explores more complex uses cases.