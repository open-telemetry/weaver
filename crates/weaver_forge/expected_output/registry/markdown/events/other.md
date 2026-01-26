# Events: `other`

This document describes the `other` events.

## `trace-exception`

This document defines the attributes used to report a single exception associated with a span.

| Property | Value |
|----------|-------|
| Event Name | `exception` |
| Stability | Development |

### Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `exception.stacktrace` | `string` | No | A stacktrace as a string in the natural representation for the language runtime. The representation is to be determined and documented by each language SIG. |
| `exception.escaped` | `boolean` | No | SHOULD be set to true if the exception event is recorded at a point where it is known that the exception is escaping the scope of the span. |
| `exception.type` | `string` | Conditional | The type of the exception (its fully-qualified class name, if applicable). The dynamic type of the exception should be preferred over the static type in languages that support it. |
| `exception.message` | `string` | Conditional | The exception message. |

