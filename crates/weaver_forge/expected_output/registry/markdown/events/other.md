# Events: `other`

This document describes the `other` events.

## `trace-exception`

This document defines the attributes used to report a single exception associated with a span.

| Property | Value |
|----------|-------|
| Event Name | `exception` |
| Stability | Development |

### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `exception.message` | `string` | Conditionally Required - Required if `exception.type` is not set, recommended otherwise. | The exception message.
 |
| `exception.type` | `string` | Conditionally Required - Required if `exception.message` is not set, recommended otherwise. | The type of the exception (its fully-qualified class name, if applicable). The dynamic type of the exception should be preferred over the static type in languages that support it.
 |
| `exception.escaped` | `boolean` | Recommended | SHOULD be set to true if the exception event is recorded at a point where it is known that the exception is escaping the scope of the span.
 |
| `exception.stacktrace` | `string` | Recommended | A stacktrace as a string in the natural representation for the language runtime. The representation is to be determined and documented by each language SIG.
 |

