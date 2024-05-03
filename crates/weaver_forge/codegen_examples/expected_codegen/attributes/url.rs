/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! Attributes describing URL.
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/attributes/attributes.rs.j2


/// Domain extracted from the `url.full`, such as "opentelemetry.io".
///
/// Notes:
///   In some cases a URL may refer to an IP and/or port directly, without a domain name. In this case, the IP address would go to the domain field. If the URL contains a [literal IPv6 address](https://www.rfc-editor.org/rfc/rfc2732#section-2) enclosed by `[` and `]`, the `[` and `]` characters should also be captured in the domain field.
#[cfg(feature = "semconv_experimental")]
pub const URL_DOMAIN: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("url.domain");




/// The file extension extracted from the `url.full`, excluding the leading dot.
///
/// Notes:
///   The file extension is only set if it exists, as not every url has a file extension. When the file name has multiple extensions `example.tar.gz`, only the last one should be captured `gz`, not `tar.gz`.
#[cfg(feature = "semconv_experimental")]
pub const URL_EXTENSION: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("url.extension");




/// The [URI fragment](https://www.rfc-editor.org/rfc/rfc3986#section-3.5) component
pub const URL_FRAGMENT: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("url.fragment");




/// Absolute URL describing a network resource according to [RFC3986](https://www.rfc-editor.org/rfc/rfc3986)
///
/// Notes:
///   For network calls, URL usually has `scheme://host[:port][path][?query][#fragment]` format, where the fragment is not transmitted over HTTP, but if it is known, it SHOULD be included nevertheless.
///   `url.full` MUST NOT contain credentials passed via URL in form of `https://username:password@www.example.com/`. In such case username and password SHOULD be redacted and attribute's value SHOULD be `https://REDACTED:REDACTED@www.example.com/`.
///   `url.full` SHOULD capture the absolute URL when it is available (or can be reconstructed). Sensitive content provided in `url.full` SHOULD be scrubbed when instrumentations can identify it.
pub const URL_FULL: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("url.full");




/// Unmodified original URL as seen in the event source.
///
/// Notes:
///   In network monitoring, the observed URL may be a full URL, whereas in access logs, the URL is often just represented as a path. This field is meant to represent the URL as it was observed, complete or not.
///   `url.original` might contain credentials passed via URL in form of `https://username:password@www.example.com/`. In such case password and username SHOULD NOT be redacted and attribute's value SHOULD remain the same.
#[cfg(feature = "semconv_experimental")]
pub const URL_ORIGINAL: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("url.original");




/// The [URI path](https://www.rfc-editor.org/rfc/rfc3986#section-3.3) component
///
/// Notes:
///   Sensitive content provided in `url.path` SHOULD be scrubbed when instrumentations can identify it.
pub const URL_PATH: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("url.path");




/// Port extracted from the `url.full`
#[cfg(feature = "semconv_experimental")]
pub const URL_PORT: crate::attributes::AttributeKey<i64> = crate::attributes::AttributeKey::new("url.port");



/// The [URI query](https://www.rfc-editor.org/rfc/rfc3986#section-3.4) component
///
/// Notes:
///   Sensitive content provided in `url.query` SHOULD be scrubbed when instrumentations can identify it.
pub const URL_QUERY: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("url.query");




/// The highest registered url domain, stripped of the subdomain.
///
/// Notes:
///   This value can be determined precisely with the [public suffix list](http://publicsuffix.org). For example, the registered domain for `foo.example.com` is `example.com`. Trying to approximate this by simply taking the last two labels will not work well for TLDs such as `co.uk`.
#[cfg(feature = "semconv_experimental")]
pub const URL_REGISTERED_DOMAIN: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("url.registered_domain");




/// The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.
pub const URL_SCHEME: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("url.scheme");




/// The subdomain portion of a fully qualified domain name includes all of the names except the host name under the registered_domain. In a partially qualified domain, or if the qualification level of the full name cannot be determined, subdomain contains all of the names below the registered domain.
///
/// Notes:
///   The subdomain portion of `www.east.mydomain.co.uk` is `east`. If the domain has multiple levels of subdomain, such as `sub2.sub1.example.com`, the subdomain field should contain `sub2.sub1`, with no trailing period.
#[cfg(feature = "semconv_experimental")]
pub const URL_SUBDOMAIN: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("url.subdomain");




/// The effective top level domain (eTLD), also known as the domain suffix, is the last part of the domain name. For example, the top level domain for example.com is `com`.
///
/// Notes:
///   This value can be determined precisely with the [public suffix list](http://publicsuffix.org).
#[cfg(feature = "semconv_experimental")]
pub const URL_TOP_LEVEL_DOMAIN: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("url.top_level_domain");


