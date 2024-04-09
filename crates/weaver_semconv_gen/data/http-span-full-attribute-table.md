<!-- semconv trace.http.common(full) -->
| Attribute  | Type | Description  | Examples  | [Requirement Level](https://opentelemetry.io/docs/specs/semconv/general/attribute-requirement-level/) | Stability |
|---|---|---|---|---|---|
| [`http.request.method`](../attributes-registry/http.md) | string | HTTP request method. [1] | `GET`; `POST`; `HEAD` | `Required` | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| [`error.type`](../attributes-registry/error.md) | string | Describes a class of error the operation ended with. [2] | `timeout`; `java.net.UnknownHostException`; `server_certificate_invalid`; `500` | `Conditionally Required` If request has ended with an error. | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| [`http.request.method_original`](../attributes-registry/http.md) | string | Original HTTP method sent by the client in the request line. | `GeT`; `ACL`; `foo` | `Conditionally Required` [3] | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| [`http.response.status_code`](../attributes-registry/http.md) | int | [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6). | `200` | `Conditionally Required` If and only if one was received/sent. | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| [`network.protocol.name`](../attributes-registry/network.md) | string | [OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent. [4] | `http`; `spdy` | `Conditionally Required` [5] | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| [`network.peer.address`](../attributes-registry/network.md) | string | Peer address of the network connection - IP address or Unix domain socket name. | `10.1.2.80`; `/tmp/my.sock` | `Recommended` | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| [`network.peer.port`](../attributes-registry/network.md) | int | Peer port number of the network connection. | `65123` | `Recommended` If `network.peer.address` is set. | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| [`network.protocol.version`](../attributes-registry/network.md) | string | Version of the protocol specified in `network.protocol.name`. [6] | `1.0`; `1.1`; `2`; `3` | `Recommended` | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| [`http.response.header.<key>`](../attributes-registry/http.md) | string[] | HTTP response headers, `<key>` being the normalized HTTP Header name (lowercase), the value being the header values. [7] | `http.response.header.content-type=["application/json"]`; `http.response.header.my-custom-header=["abc", "def"]` | `Opt-In` | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |
| [`network.transport`](../attributes-registry/network.md) | string | [OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication). [8] | `tcp`; `udp` | `Opt-In` | ![Stable](https://img.shields.io/badge/-stable-lightgreen) |

**[1]:** HTTP request method value SHOULD be "known" to the instrumentation.
By default, this convention defines "known" methods as the ones listed in [RFC9110](https://www.rfc-editor.org/rfc/rfc9110.html#name-methods)
and the PATCH method defined in [RFC5789](https://www.rfc-editor.org/rfc/rfc5789.html).

If the HTTP request method is not known to instrumentation, it MUST set the `http.request.method` attribute to `_OTHER`.

If the HTTP instrumentation could end up converting valid HTTP request methods to `_OTHER`, then it MUST provide a way to override
the list of known HTTP methods. If this override is done via environment variable, then the environment variable MUST be named
OTEL_INSTRUMENTATION_HTTP_KNOWN_METHODS and support a comma-separated list of case-sensitive known HTTP methods
(this list MUST be a full override of the default known method, it is not a list of known methods in addition to the defaults).

HTTP method names are case-sensitive and `http.request.method` attribute value MUST match a known HTTP method name exactly.
Instrumentations for specific web frameworks that consider HTTP methods to be case insensitive, SHOULD populate a canonical equivalent.
Tracing instrumentations that do so, MUST also set `http.request.method_original` to the original value.

**[2]:** If the request fails with an error before response status code was sent or received,
`error.type` SHOULD be set to exception type (its fully-qualified class name, if applicable)
or a component-specific low cardinality error identifier.

If response status code was sent or received and status indicates an error according to [HTTP span status definition](/docs/http/http-spans.md),
`error.type` SHOULD be set to the status code number (represented as a string), an exception type (if thrown) or a component-specific error identifier.

The `error.type` value SHOULD be predictable and SHOULD have low cardinality.
Instrumentations SHOULD document the list of errors they report.

The cardinality of `error.type` within one instrumentation library SHOULD be low, but
telemetry consumers that aggregate data from multiple instrumentation libraries and applications
should be prepared for `error.type` to have high cardinality at query time, when no
additional filters are applied.

If the request has completed successfully, instrumentations SHOULD NOT set `error.type`.

**[3]:** If and only if it's different than `http.request.method`.

**[4]:** The value SHOULD be normalized to lowercase.

**[5]:** If not `http` and `network.protocol.version` is set.

**[6]:** `network.protocol.version` refers to the version of the protocol used and might be different from the protocol client's version. If the HTTP client has a version of `0.27.2`, but sends HTTP version `1.1`, this attribute should be set to `1.1`.

**[7]:** Instrumentations SHOULD require an explicit configuration of which headers are to be captured. Including all response headers can be a security risk - explicit configuration helps avoid leaking sensitive information.
Users MAY explicitly configure instrumentations to capture them even though it is not recommended.
The attribute value MUST consist of either multiple header values as an array of strings or a single-item array containing a possibly comma-concatenated string, depending on the way the HTTP library provides access to headers.

**[8]:** Generally `tcp` for `HTTP/1.0`, `HTTP/1.1`, and `HTTP/2`. Generally `udp` for `HTTP/3`. Other obscure implementations are possible.

The following attributes can be important for making sampling decisions and SHOULD be provided **at span creation time** (if provided at all):

* [`http.request.method`](../attributes-registry/http.md)

`http.request.method` has the following list of well-known values. If one of them applies, then the respective value MUST be used; otherwise, a custom value MAY be used.

| Value  | Description | Stability |
|---|---|---|
| `CONNECT` | CONNECT method. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `DELETE` | DELETE method. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `GET` | GET method. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `HEAD` | HEAD method. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `OPTIONS` | OPTIONS method. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `PATCH` | PATCH method. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `POST` | POST method. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `PUT` | PUT method. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `TRACE` | TRACE method. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `_OTHER` | Any HTTP method that the instrumentation has no prior knowledge of. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |

`error.type` has the following list of well-known values. If one of them applies, then the respective value MUST be used; otherwise, a custom value MAY be used.

| Value  | Description | Stability |
|---|---|---|
| `_OTHER` | A fallback error value to be used when the instrumentation doesn't define a custom value. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |

`network.transport` has the following list of well-known values. If one of them applies, then the respective value MUST be used; otherwise, a custom value MAY be used.

| Value  | Description | Stability |
|---|---|---|
| `tcp` | TCP | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `udp` | UDP | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `pipe` | Named or anonymous pipe. | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
| `unix` | Unix domain socket | ![Experimental](https://img.shields.io/badge/-experimental-blue) |
<!-- endsemconv -->
