# Spans: `http`

This document describes the `http` spans.

## `span.http.client`

HTTP client span

| Property | Value |
|----------|-------|
| Span Kind | client |
| Stability | Stable |

### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `http.request.method` | Enum | Required | HTTP request method.
 |
| `server.address` | `string` | Required | Some HTTP specific description
 |
| `server.port` | `int` | Required | Some HTTP specific description
 |
| `url.full` | `string` | Required | Absolute URL describing a network resource according to [RFC3986](https://www.rfc-editor.org/rfc/rfc3986)
 |
| `http.response.status_code` | `int` | Recommended | [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6).
 |

