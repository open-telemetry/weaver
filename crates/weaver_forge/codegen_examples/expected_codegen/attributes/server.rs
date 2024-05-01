/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! These attributes may be used to describe the server in a connection-based network interaction where there is one side that initiates the connection (the client is the side that initiates the connection). This covers all TCP network interactions since TCP is connection-based and one side initiates the connection (an exception is made for peer-to-peer communication over TCP where the "user-facing" surface of the protocol / API doesn't expose a clear notion of client and server). This also covers UDP network interactions where one side initiates the interaction, e.g. QUIC (HTTP/3) and DNS.
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/attributes/attributes.rs.j2


/// Server domain name if available without reverse DNS lookup; otherwise, IP address or Unix domain socket name.
///
/// Notes:
///   When observed from the client side, and when communicating through an intermediary, `server.address` SHOULD represent the server address behind any intermediaries, for example proxies, if it's available.
pub const SERVER_ADDRESS: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("server.address");




/// Server port number.
///
/// Notes:
///   When observed from the client side, and when communicating through an intermediary, `server.port` SHOULD represent the server port behind any intermediaries, for example proxies, if it's available.
pub const SERVER_PORT: crate::attributes::AttributeKey<i64> = crate::attributes::AttributeKey::new("server.port");


