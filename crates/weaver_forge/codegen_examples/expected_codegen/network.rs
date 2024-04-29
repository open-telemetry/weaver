/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! These attributes may be used for any network related operation.
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/lib.rs.j2

use opentelemetry::StringValue;
use crate::AttributeKey;


/// The ISO 3166-1 alpha-2 2-character country code associated with the mobile carrier network.
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CARRIER_ICC: AttributeKey<StringValue> = AttributeKey::new("network.carrier.icc");



/// The mobile carrier country code.
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CARRIER_MCC: AttributeKey<StringValue> = AttributeKey::new("network.carrier.mcc");



/// The mobile carrier network code.
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CARRIER_MNC: AttributeKey<StringValue> = AttributeKey::new("network.carrier.mnc");



/// The name of the mobile carrier.
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CARRIER_NAME: AttributeKey<StringValue> = AttributeKey::new("network.carrier.name");



/// This describes more details regarding the connection.type. It may be the type of cell technology connection, but it could be used for describing details about a wifi connection.
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CONNECTION_SUBTYPE: AttributeKey<NetworkConnectionSubtype> = AttributeKey::new("network.connection.subtype");


/// This describes more details regarding the connection.type. It may be the type of cell technology connection, but it could be used for describing details about a wifi connection.
#[non_exhaustive]
pub enum NetworkConnectionSubtype {
    /// GPRS
    #[cfg(feature = "semconv_experimental")] 
    Gprs,
    /// EDGE
    #[cfg(feature = "semconv_experimental")] 
    Edge,
    /// UMTS
    #[cfg(feature = "semconv_experimental")] 
    Umts,
    /// CDMA
    #[cfg(feature = "semconv_experimental")] 
    Cdma,
    /// EVDO Rel. 0
    #[cfg(feature = "semconv_experimental")] 
    Evdo0,
    /// EVDO Rev. A
    #[cfg(feature = "semconv_experimental")] 
    EvdoA,
    /// CDMA2000 1XRTT
    #[cfg(feature = "semconv_experimental")] 
    Cdma20001Xrtt,
    /// HSDPA
    #[cfg(feature = "semconv_experimental")] 
    Hsdpa,
    /// HSUPA
    #[cfg(feature = "semconv_experimental")] 
    Hsupa,
    /// HSPA
    #[cfg(feature = "semconv_experimental")] 
    Hspa,
    /// IDEN
    #[cfg(feature = "semconv_experimental")] 
    Iden,
    /// EVDO Rev. B
    #[cfg(feature = "semconv_experimental")] 
    EvdoB,
    /// LTE
    #[cfg(feature = "semconv_experimental")] 
    Lte,
    /// EHRPD
    #[cfg(feature = "semconv_experimental")] 
    Ehrpd,
    /// HSPAP
    #[cfg(feature = "semconv_experimental")] 
    Hspap,
    /// GSM
    #[cfg(feature = "semconv_experimental")] 
    Gsm,
    /// TD-SCDMA
    #[cfg(feature = "semconv_experimental")] 
    TdScdma,
    /// IWLAN
    #[cfg(feature = "semconv_experimental")] 
    Iwlan,
    /// 5G NR (New Radio)
    #[cfg(feature = "semconv_experimental")] 
    Nr,
    /// 5G NRNSA (New Radio Non-Standalone)
    #[cfg(feature = "semconv_experimental")] 
    Nrnsa,
    /// LTE CA
    #[cfg(feature = "semconv_experimental")] 
    LteCa,

}


/// The internet connection type.
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CONNECTION_TYPE: AttributeKey<NetworkConnectionType> = AttributeKey::new("network.connection.type");


/// The internet connection type.
#[non_exhaustive]
pub enum NetworkConnectionType {

    #[cfg(feature = "semconv_experimental")] 
    Wifi,

    #[cfg(feature = "semconv_experimental")] 
    Wired,

    #[cfg(feature = "semconv_experimental")] 
    Cell,

    #[cfg(feature = "semconv_experimental")] 
    Unavailable,

    #[cfg(feature = "semconv_experimental")] 
    Unknown,

}


/// Local address of the network connection - IP address or Unix domain socket name.
pub const NETWORK_LOCAL_ADDRESS: AttributeKey<StringValue> = AttributeKey::new("network.local.address");



/// Local port number of the network connection.
pub const NETWORK_LOCAL_PORT: AttributeKey<i64> = AttributeKey::new("network.local.port");


/// Peer address of the network connection - IP address or Unix domain socket name.
pub const NETWORK_PEER_ADDRESS: AttributeKey<StringValue> = AttributeKey::new("network.peer.address");



/// Peer port number of the network connection.
pub const NETWORK_PEER_PORT: AttributeKey<i64> = AttributeKey::new("network.peer.port");


/// [OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.
///
/// Notes:
///   The value SHOULD be normalized to lowercase.
pub const NETWORK_PROTOCOL_NAME: AttributeKey<StringValue> = AttributeKey::new("network.protocol.name");



/// The actual version of the protocol used for network communication.
///
/// Notes:
///   If protocol version is subject to negotiation (for example using [ALPN](https://www.rfc-editor.org/rfc/rfc7301.html)), this attribute SHOULD be set to the negotiated version. If the actual protocol version is not known, this attribute SHOULD NOT be set.
pub const NETWORK_PROTOCOL_VERSION: AttributeKey<StringValue> = AttributeKey::new("network.protocol.version");



/// [OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication).
///
/// Notes:
///   The value SHOULD be normalized to lowercase.
///   
///   Consider always setting the transport when setting a port number, since
///   a port number is ambiguous without knowing the transport. For example
///   different processes could be listening on TCP port 12345 and UDP port 12345.
pub const NETWORK_TRANSPORT: AttributeKey<NetworkTransport> = AttributeKey::new("network.transport");


/// [OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication).
#[non_exhaustive]
pub enum NetworkTransport {
    /// TCP
    Tcp,
    /// UDP
    Udp,
    /// Named or anonymous pipe.
    Pipe,
    /// Unix domain socket
    Unix,

}


/// [OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.
///
/// Notes:
///   The value SHOULD be normalized to lowercase.
pub const NETWORK_TYPE: AttributeKey<NetworkType> = AttributeKey::new("network.type");


/// [OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.
#[non_exhaustive]
pub enum NetworkType {
    /// IPv4
    Ipv4,
    /// IPv6
    Ipv6,

}


/// The network IO operation direction.
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_IO_DIRECTION: AttributeKey<NetworkIoDirection> = AttributeKey::new("network.io.direction");


/// The network IO operation direction.
#[non_exhaustive]
pub enum NetworkIoDirection {

    #[cfg(feature = "semconv_experimental")] 
    Transmit,

    #[cfg(feature = "semconv_experimental")] 
    Receive,

}

