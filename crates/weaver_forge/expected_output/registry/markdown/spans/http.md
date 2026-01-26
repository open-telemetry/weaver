# Spans: `http`

This document describes the `http` spans.

## `span.http.client`

HTTP client span

| Property | Value |
|----------|-------|
| Span Kind | client |
| Stability | Stable |

### Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `http.response.status_code` | `int` | No | [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6). |
| `http.request.method` | Enum | **Yes** | HTTP request method. |
| `server.address` | `string` | **Yes** | Some HTTP specific description |
| `server.port` | `int` | **Yes** | Some HTTP specific description |
| `url.full` | `string` | **Yes** | Absolute URL describing a network resource according to [RFC3986](https://www.rfc-editor.org/rfc/rfc3986) |

