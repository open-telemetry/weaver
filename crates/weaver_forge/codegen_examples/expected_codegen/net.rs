/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! These attributes may be used for any network related operation.
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/lib.rs.j2

use opentelemetry::StringValue;
use crate::AttributeKey;


/// Deprecated, no replacement at this time.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Removed.")]
pub const NET_SOCK_PEER_NAME: AttributeKey<StringValue> = AttributeKey::new("net.sock.peer.name");



/// Deprecated, use `network.peer.address`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.peer.address`.")]
pub const NET_SOCK_PEER_ADDR: AttributeKey<StringValue> = AttributeKey::new("net.sock.peer.addr");



/// Deprecated, use `network.peer.port`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.peer.port`.")]
pub const NET_SOCK_PEER_PORT: AttributeKey<i64> = AttributeKey::new("net.sock.peer.port");


/// Deprecated, use `server.address` on client spans and `client.address` on server spans.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `server.address` on client spans and `client.address` on server spans.")]
pub const NET_PEER_NAME: AttributeKey<StringValue> = AttributeKey::new("net.peer.name");



/// Deprecated, use `server.port` on client spans and `client.port` on server spans.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `server.port` on client spans and `client.port` on server spans.")]
pub const NET_PEER_PORT: AttributeKey<i64> = AttributeKey::new("net.peer.port");


/// Deprecated, use `server.address`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `server.address`.")]
pub const NET_HOST_NAME: AttributeKey<StringValue> = AttributeKey::new("net.host.name");



/// Deprecated, use `server.port`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `server.port`.")]
pub const NET_HOST_PORT: AttributeKey<i64> = AttributeKey::new("net.host.port");


/// Deprecated, use `network.local.address`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.local.address`.")]
pub const NET_SOCK_HOST_ADDR: AttributeKey<StringValue> = AttributeKey::new("net.sock.host.addr");



/// Deprecated, use `network.local.port`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.local.port`.")]
pub const NET_SOCK_HOST_PORT: AttributeKey<i64> = AttributeKey::new("net.sock.host.port");


/// Deprecated, use `network.transport`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.transport`.")]
pub const NET_TRANSPORT: AttributeKey<NetTransport> = AttributeKey::new("net.transport");


/// Deprecated, use `network.transport`.
#[non_exhaustive]
pub enum NetTransport {

    #[cfg(feature = "semconv_experimental")] 
    IpTcp,

    #[cfg(feature = "semconv_experimental")] 
    IpUdp,
    /// Named or anonymous pipe.
    #[cfg(feature = "semconv_experimental")] 
    Pipe,
    /// In-process communication.    /// Signals that there is only in-process communication not using a "real" network protocol in cases where network attributes would normally be expected. Usually all other network attributes can be left out in that case.
    #[cfg(feature = "semconv_experimental")] 
    Inproc,
    /// Something else (non IP-based).
    #[cfg(feature = "semconv_experimental")] 
    Other,

}


/// Deprecated, use `network.protocol.name`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.protocol.name`.")]
pub const NET_PROTOCOL_NAME: AttributeKey<StringValue> = AttributeKey::new("net.protocol.name");



/// Deprecated, use `network.protocol.version`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.protocol.version`.")]
pub const NET_PROTOCOL_VERSION: AttributeKey<StringValue> = AttributeKey::new("net.protocol.version");



/// Deprecated, use `network.transport` and `network.type`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Split to `network.transport` and `network.type`.")]
pub const NET_SOCK_FAMILY: AttributeKey<NetSockFamily> = AttributeKey::new("net.sock.family");


/// Deprecated, use `network.transport` and `network.type`.
#[non_exhaustive]
pub enum NetSockFamily {
    /// IPv4 address
    #[cfg(feature = "semconv_experimental")] 
    Inet,
    /// IPv6 address
    #[cfg(feature = "semconv_experimental")] 
    Inet6,
    /// Unix domain socket path
    #[cfg(feature = "semconv_experimental")] 
    Unix,

}

