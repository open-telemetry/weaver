/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! This document defines semantic convention attributes in the HTTP namespace.
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/lib.rs.j2

use opentelemetry::StringValue;
use crate::AttributeKey;


/// The size of the request payload body in bytes. This is the number of bytes transferred excluding headers and is often, but not always, present as the [Content-Length](https://www.rfc-editor.org/rfc/rfc9110.html#field.content-length) header. For requests using transport encoding, this should be the compressed size.
#[cfg(feature = "semconv_experimental")]
pub const HTTP_REQUEST_BODY_SIZE: AttributeKey<i64> = AttributeKey::new("http.request.body.size");


/// HTTP request headers, `<key>` being the normalized HTTP Header name (lowercase), the value being the header values.
///
/// Notes:
///   Instrumentations SHOULD require an explicit configuration of which headers are to be captured. Including all request headers can be a security risk - explicit configuration helps avoid leaking sensitive information.
///   The `User-Agent` header is already captured in the `user_agent.original` attribute. Users MAY explicitly configure instrumentations to capture them even though it is not recommended.
///   The attribute value MUST consist of either multiple header values as an array of strings or a single-item array containing a possibly comma-concatenated string, depending on the way the HTTP library provides access to headers.
pub const HTTP_REQUEST_HEADER: AttributeKey<Vec<String>> = AttributeKey::new("http.request.header");


/// HTTP request method.
///
/// Notes:
///   HTTP request method value SHOULD be "known" to the instrumentation.
///   By default, this convention defines "known" methods as the ones listed in [RFC9110](https://www.rfc-editor.org/rfc/rfc9110.html#name-methods)
///   and the PATCH method defined in [RFC5789](https://www.rfc-editor.org/rfc/rfc5789.html).
///   
///   If the HTTP request method is not known to instrumentation, it MUST set the `http.request.method` attribute to `_OTHER`.
///   
///   If the HTTP instrumentation could end up converting valid HTTP request methods to `_OTHER`, then it MUST provide a way to override
///   the list of known HTTP methods. If this override is done via environment variable, then the environment variable MUST be named
///   OTEL_INSTRUMENTATION_HTTP_KNOWN_METHODS and support a comma-separated list of case-sensitive known HTTP methods
///   (this list MUST be a full override of the default known method, it is not a list of known methods in addition to the defaults).
///   
///   HTTP method names are case-sensitive and `http.request.method` attribute value MUST match a known HTTP method name exactly.
///   Instrumentations for specific web frameworks that consider HTTP methods to be case insensitive, SHOULD populate a canonical equivalent.
///   Tracing instrumentations that do so, MUST also set `http.request.method_original` to the original value.
pub const HTTP_REQUEST_METHOD: AttributeKey<HttpRequestMethod> = AttributeKey::new("http.request.method");


/// HTTP request method.
#[non_exhaustive]
pub enum HttpRequestMethod {
    /// CONNECT method.
    Connect,
    /// DELETE method.
    Delete,
    /// GET method.
    Get,
    /// HEAD method.
    Head,
    /// OPTIONS method.
    Options,
    /// PATCH method.
    Patch,
    /// POST method.
    Post,
    /// PUT method.
    Put,
    /// TRACE method.
    Trace,
    /// Any HTTP method that the instrumentation has no prior knowledge of.
    Other,

}


/// Original HTTP method sent by the client in the request line.
pub const HTTP_REQUEST_METHOD_ORIGINAL: AttributeKey<StringValue> = AttributeKey::new("http.request.method_original");



/// The ordinal number of request resending attempt (for any reason, including redirects).
///
/// Notes:
///   The resend count SHOULD be updated each time an HTTP request gets resent by the client, regardless of what was the cause of the resending (e.g. redirection, authorization failure, 503 Server Unavailable, network issues, or any other).
pub const HTTP_REQUEST_RESEND_COUNT: AttributeKey<i64> = AttributeKey::new("http.request.resend_count");


/// The total size of the request in bytes. This should be the total number of bytes sent over the wire, including the request line (HTTP/1.1), framing (HTTP/2 and HTTP/3), headers, and request body if any.
#[cfg(feature = "semconv_experimental")]
pub const HTTP_REQUEST_SIZE: AttributeKey<i64> = AttributeKey::new("http.request.size");


/// The size of the response payload body in bytes. This is the number of bytes transferred excluding headers and is often, but not always, present as the [Content-Length](https://www.rfc-editor.org/rfc/rfc9110.html#field.content-length) header. For requests using transport encoding, this should be the compressed size.
#[cfg(feature = "semconv_experimental")]
pub const HTTP_RESPONSE_BODY_SIZE: AttributeKey<i64> = AttributeKey::new("http.response.body.size");


/// HTTP response headers, `<key>` being the normalized HTTP Header name (lowercase), the value being the header values.
///
/// Notes:
///   Instrumentations SHOULD require an explicit configuration of which headers are to be captured. Including all response headers can be a security risk - explicit configuration helps avoid leaking sensitive information.
///   Users MAY explicitly configure instrumentations to capture them even though it is not recommended.
///   The attribute value MUST consist of either multiple header values as an array of strings or a single-item array containing a possibly comma-concatenated string, depending on the way the HTTP library provides access to headers.
pub const HTTP_RESPONSE_HEADER: AttributeKey<Vec<String>> = AttributeKey::new("http.response.header");


/// The total size of the response in bytes. This should be the total number of bytes sent over the wire, including the status line (HTTP/1.1), framing (HTTP/2 and HTTP/3), headers, and response body and trailers if any.
#[cfg(feature = "semconv_experimental")]
pub const HTTP_RESPONSE_SIZE: AttributeKey<i64> = AttributeKey::new("http.response.size");


/// [HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6).
pub const HTTP_RESPONSE_STATUS_CODE: AttributeKey<i64> = AttributeKey::new("http.response.status_code");


/// The matched route, that is, the path template in the format used by the respective server framework.
///
/// Notes:
///   MUST NOT be populated when this is not supported by the HTTP server framework as the route attribute should have low-cardinality and the URI path can NOT substitute it.
///   SHOULD include the [application root](/docs/http/http-spans.md#http-server-definitions) if there is one.
pub const HTTP_ROUTE: AttributeKey<StringValue> = AttributeKey::new("http.route");



/// State of the HTTP connection in the HTTP connection pool.
#[cfg(feature = "semconv_experimental")]
pub const HTTP_CONNECTION_STATE: AttributeKey<HttpConnectionState> = AttributeKey::new("http.connection.state");


/// State of the HTTP connection in the HTTP connection pool.
#[non_exhaustive]
pub enum HttpConnectionState {
    /// active state.
    #[cfg(feature = "semconv_experimental")] 
    Active,
    /// idle state.
    #[cfg(feature = "semconv_experimental")] 
    Idle,

}

