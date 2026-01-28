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

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `server.address` | `string` | **Yes** | Some HTTP specific description |
| `server.port` | `int` | **Yes** | Some HTTP specific description |
| `http.request.method` | Enum | No | HTTP request method. |
| `http.response.status_code` | `int` | No | [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6). |
| `url.scheme` | `string` | No | The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol. |

