# Metrics: `http`

This document describes the `http` metrics.

## `http.client.request.duration`

Duration of HTTP client requests.

| Property | Value |
|----------|-------|
| Instrument | histogram |
| Unit | `s` |
| Stability | Stable |

### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `server.address` | `string` | Required | Some HTTP specific description
 |
| `server.port` | `int` | Required | Some HTTP specific description
 |
| `http.request.method` | Enum | Recommended | HTTP request method.
 |
| `http.response.status_code` | `int` | Recommended | [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6).
 |
| `url.scheme` | `string` | Opt-In | The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.
 |

