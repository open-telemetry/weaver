/*
 * Copyright The OpenTelemetry Authors
 * SPDX-License-Identifier: Apache-2.0
 */

//! These attributes may be used for any network related operation.
//! DO NOT EDIT, this is an Auto-generated file from templates/registry/rust/attributes/attributes.rs.j2

/// Deprecated, use `server.address`.
///
/// Examples:
/// - example.com
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `server.address`.")]
pub const NET_HOST_NAME: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("net.host.name");

/// Deprecated, use `server.port`.
///
/// Examples:
/// - 8080
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `server.port`.")]
pub const NET_HOST_PORT: crate::attributes::AttributeKey<i64> = crate::attributes::AttributeKey::new("net.host.port");

/// Deprecated, use `server.address` on client spans and `client.address` on server spans.
///
/// Examples:
/// - example.com
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `server.address` on client spans and `client.address` on server spans.")]
pub const NET_PEER_NAME: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("net.peer.name");

/// Deprecated, use `server.port` on client spans and `client.port` on server spans.
///
/// Examples:
/// - 8080
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `server.port` on client spans and `client.port` on server spans.")]
pub const NET_PEER_PORT: crate::attributes::AttributeKey<i64> = crate::attributes::AttributeKey::new("net.peer.port");

/// Deprecated, use `network.protocol.name`.
///
/// Examples:
/// - amqp
/// - http
/// - mqtt
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.protocol.name`.")]
pub const NET_PROTOCOL_NAME: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("net.protocol.name");

/// Deprecated, use `network.protocol.version`.
///
/// Example: 3.1.1
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.protocol.version`.")]
pub const NET_PROTOCOL_VERSION: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("net.protocol.version");

/// Deprecated, use `network.transport` and `network.type`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Split to `network.transport` and `network.type`.")]
pub const NET_SOCK_FAMILY: crate::attributes::AttributeKey<NetSockFamily> = crate::attributes::AttributeKey::new("net.sock.family");

/// Deprecated, use `network.transport` and `network.type`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NetSockFamily {    /// IPv4 address
    #[cfg(feature = "semconv_experimental")] 
    Inet,    /// IPv6 address
    #[cfg(feature = "semconv_experimental")] 
    Inet6,    /// Unix domain socket path
    #[cfg(feature = "semconv_experimental")] 
    Unix,
    /// This variant allows defining a custom entry in the enum.
    _Custom(String),
}

impl NetSockFamily {
    /// Returns the string representation of the [`NetSockFamily`].
    pub fn as_str(&self) -> &str {
        match self {
            #[cfg(feature = "semconv_experimental")] 
            NetSockFamily::Inet => "inet",
            #[cfg(feature = "semconv_experimental")] 
            NetSockFamily::Inet6 => "inet6",
            #[cfg(feature = "semconv_experimental")] 
            NetSockFamily::Unix => "unix",
            NetSockFamily::_Custom(v) => v.as_str(),
            // Without this default case, the match expression would not
            // contain any variants if all variants are annotated with the
            // 'semconv_experimental' feature and the feature is not enabled.
            #[allow(unreachable_patterns)]
            _ => unreachable!(),
        }
    }
}

impl core::fmt::Display for NetSockFamily {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl crate::attributes::AttributeKey<NetSockFamily> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: &NetSockFamily) -> opentelemetry::KeyValue {
        opentelemetry::KeyValue::new(self.key.clone(), v.to_string())
    }
}

/// Deprecated, use `network.local.address`.
///
/// Examples:
/// - /var/my.sock
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.local.address`.")]
pub const NET_SOCK_HOST_ADDR: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("net.sock.host.addr");

/// Deprecated, use `network.local.port`.
///
/// Examples:
/// - 8080
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.local.port`.")]
pub const NET_SOCK_HOST_PORT: crate::attributes::AttributeKey<i64> = crate::attributes::AttributeKey::new("net.sock.host.port");

/// Deprecated, use `network.peer.address`.
///
/// Examples:
/// - 192.168.0.1
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.peer.address`.")]
pub const NET_SOCK_PEER_ADDR: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("net.sock.peer.addr");

/// Deprecated, no replacement at this time.
///
/// Examples:
/// - /var/my.sock
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Removed.")]
pub const NET_SOCK_PEER_NAME: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("net.sock.peer.name");

/// Deprecated, use `network.peer.port`.
///
/// Examples:
/// - 65531
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.peer.port`.")]
pub const NET_SOCK_PEER_PORT: crate::attributes::AttributeKey<i64> = crate::attributes::AttributeKey::new("net.sock.peer.port");

/// Deprecated, use `network.transport`.
#[cfg(feature = "semconv_experimental")]
#[deprecated(note="Replaced by `network.transport`.")]
pub const NET_TRANSPORT: crate::attributes::AttributeKey<NetTransport> = crate::attributes::AttributeKey::new("net.transport");

/// Deprecated, use `network.transport`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NetTransport {
    #[cfg(feature = "semconv_experimental")] 
    IpTcp,
    #[cfg(feature = "semconv_experimental")] 
    IpUdp,    /// Named or anonymous pipe.
    #[cfg(feature = "semconv_experimental")] 
    Pipe,    /// In-process communication.    /// Signals that there is only in-process communication not using a "real" network protocol in cases where network attributes would normally be expected. Usually all other network attributes can be left out in that case.
    #[cfg(feature = "semconv_experimental")] 
    Inproc,    /// Something else (non IP-based).
    #[cfg(feature = "semconv_experimental")] 
    Other,
    /// This variant allows defining a custom entry in the enum.
    _Custom(String),
}

impl NetTransport {
    /// Returns the string representation of the [`NetTransport`].
    pub fn as_str(&self) -> &str {
        match self {
            #[cfg(feature = "semconv_experimental")] 
            NetTransport::IpTcp => "ip_tcp",
            #[cfg(feature = "semconv_experimental")] 
            NetTransport::IpUdp => "ip_udp",
            #[cfg(feature = "semconv_experimental")] 
            NetTransport::Pipe => "pipe",
            #[cfg(feature = "semconv_experimental")] 
            NetTransport::Inproc => "inproc",
            #[cfg(feature = "semconv_experimental")] 
            NetTransport::Other => "other",
            NetTransport::_Custom(v) => v.as_str(),
            // Without this default case, the match expression would not
            // contain any variants if all variants are annotated with the
            // 'semconv_experimental' feature and the feature is not enabled.
            #[allow(unreachable_patterns)]
            _ => unreachable!(),
        }
    }
}

impl core::fmt::Display for NetTransport {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl crate::attributes::AttributeKey<NetTransport> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: &NetTransport) -> opentelemetry::KeyValue {
        opentelemetry::KeyValue::new(self.key.clone(), v.to_string())
    }
}

/// The ISO 3166-1 alpha-2 2-character country code associated with the mobile carrier network.
///
/// Example: DE
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CARRIER_ICC: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("network.carrier.icc");

/// The mobile carrier country code.
///
/// Example: 310
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CARRIER_MCC: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("network.carrier.mcc");

/// The mobile carrier network code.
///
/// Example: 001
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CARRIER_MNC: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("network.carrier.mnc");

/// The name of the mobile carrier.
///
/// Example: sprint
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CARRIER_NAME: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("network.carrier.name");

/// This describes more details regarding the connection.type. It may be the type of cell technology connection, but it could be used for describing details about a wifi connection.
///
/// Example: LTE
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CONNECTION_SUBTYPE: crate::attributes::AttributeKey<NetworkConnectionSubtype> = crate::attributes::AttributeKey::new("network.connection.subtype");

/// This describes more details regarding the connection.type. It may be the type of cell technology connection, but it could be used for describing details about a wifi connection.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NetworkConnectionSubtype {    /// GPRS
    #[cfg(feature = "semconv_experimental")] 
    Gprs,    /// EDGE
    #[cfg(feature = "semconv_experimental")] 
    Edge,    /// UMTS
    #[cfg(feature = "semconv_experimental")] 
    Umts,    /// CDMA
    #[cfg(feature = "semconv_experimental")] 
    Cdma,    /// EVDO Rel. 0
    #[cfg(feature = "semconv_experimental")] 
    Evdo0,    /// EVDO Rev. A
    #[cfg(feature = "semconv_experimental")] 
    EvdoA,    /// CDMA2000 1XRTT
    #[cfg(feature = "semconv_experimental")] 
    Cdma20001Xrtt,    /// HSDPA
    #[cfg(feature = "semconv_experimental")] 
    Hsdpa,    /// HSUPA
    #[cfg(feature = "semconv_experimental")] 
    Hsupa,    /// HSPA
    #[cfg(feature = "semconv_experimental")] 
    Hspa,    /// IDEN
    #[cfg(feature = "semconv_experimental")] 
    Iden,    /// EVDO Rev. B
    #[cfg(feature = "semconv_experimental")] 
    EvdoB,    /// LTE
    #[cfg(feature = "semconv_experimental")] 
    Lte,    /// EHRPD
    #[cfg(feature = "semconv_experimental")] 
    Ehrpd,    /// HSPAP
    #[cfg(feature = "semconv_experimental")] 
    Hspap,    /// GSM
    #[cfg(feature = "semconv_experimental")] 
    Gsm,    /// TD-SCDMA
    #[cfg(feature = "semconv_experimental")] 
    TdScdma,    /// IWLAN
    #[cfg(feature = "semconv_experimental")] 
    Iwlan,    /// 5G NR (New Radio)
    #[cfg(feature = "semconv_experimental")] 
    Nr,    /// 5G NRNSA (New Radio Non-Standalone)
    #[cfg(feature = "semconv_experimental")] 
    Nrnsa,    /// LTE CA
    #[cfg(feature = "semconv_experimental")] 
    LteCa,
    /// This variant allows defining a custom entry in the enum.
    _Custom(String),
}

impl NetworkConnectionSubtype {
    /// Returns the string representation of the [`NetworkConnectionSubtype`].
    pub fn as_str(&self) -> &str {
        match self {
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Gprs => "gprs",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Edge => "edge",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Umts => "umts",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Cdma => "cdma",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Evdo0 => "evdo_0",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::EvdoA => "evdo_a",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Cdma20001Xrtt => "cdma2000_1xrtt",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Hsdpa => "hsdpa",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Hsupa => "hsupa",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Hspa => "hspa",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Iden => "iden",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::EvdoB => "evdo_b",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Lte => "lte",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Ehrpd => "ehrpd",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Hspap => "hspap",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Gsm => "gsm",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::TdScdma => "td_scdma",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Iwlan => "iwlan",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Nr => "nr",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::Nrnsa => "nrnsa",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionSubtype::LteCa => "lte_ca",
            NetworkConnectionSubtype::_Custom(v) => v.as_str(),
            // Without this default case, the match expression would not
            // contain any variants if all variants are annotated with the
            // 'semconv_experimental' feature and the feature is not enabled.
            #[allow(unreachable_patterns)]
            _ => unreachable!(),
        }
    }
}

impl core::fmt::Display for NetworkConnectionSubtype {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl crate::attributes::AttributeKey<NetworkConnectionSubtype> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: &NetworkConnectionSubtype) -> opentelemetry::KeyValue {
        opentelemetry::KeyValue::new(self.key.clone(), v.to_string())
    }
}

/// The internet connection type.
///
/// Example: wifi
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_CONNECTION_TYPE: crate::attributes::AttributeKey<NetworkConnectionType> = crate::attributes::AttributeKey::new("network.connection.type");

/// The internet connection type.
#[derive(Debug, Clone)]
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
    /// This variant allows defining a custom entry in the enum.
    _Custom(String),
}

impl NetworkConnectionType {
    /// Returns the string representation of the [`NetworkConnectionType`].
    pub fn as_str(&self) -> &str {
        match self {
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionType::Wifi => "wifi",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionType::Wired => "wired",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionType::Cell => "cell",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionType::Unavailable => "unavailable",
            #[cfg(feature = "semconv_experimental")] 
            NetworkConnectionType::Unknown => "unknown",
            NetworkConnectionType::_Custom(v) => v.as_str(),
            // Without this default case, the match expression would not
            // contain any variants if all variants are annotated with the
            // 'semconv_experimental' feature and the feature is not enabled.
            #[allow(unreachable_patterns)]
            _ => unreachable!(),
        }
    }
}

impl core::fmt::Display for NetworkConnectionType {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl crate::attributes::AttributeKey<NetworkConnectionType> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: &NetworkConnectionType) -> opentelemetry::KeyValue {
        opentelemetry::KeyValue::new(self.key.clone(), v.to_string())
    }
}

/// The network IO operation direction.
///
/// Examples:
/// - transmit
#[cfg(feature = "semconv_experimental")]
pub const NETWORK_IO_DIRECTION: crate::attributes::AttributeKey<NetworkIoDirection> = crate::attributes::AttributeKey::new("network.io.direction");

/// The network IO operation direction.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NetworkIoDirection {
    #[cfg(feature = "semconv_experimental")] 
    Transmit,
    #[cfg(feature = "semconv_experimental")] 
    Receive,
    /// This variant allows defining a custom entry in the enum.
    _Custom(String),
}

impl NetworkIoDirection {
    /// Returns the string representation of the [`NetworkIoDirection`].
    pub fn as_str(&self) -> &str {
        match self {
            #[cfg(feature = "semconv_experimental")] 
            NetworkIoDirection::Transmit => "transmit",
            #[cfg(feature = "semconv_experimental")] 
            NetworkIoDirection::Receive => "receive",
            NetworkIoDirection::_Custom(v) => v.as_str(),
            // Without this default case, the match expression would not
            // contain any variants if all variants are annotated with the
            // 'semconv_experimental' feature and the feature is not enabled.
            #[allow(unreachable_patterns)]
            _ => unreachable!(),
        }
    }
}

impl core::fmt::Display for NetworkIoDirection {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl crate::attributes::AttributeKey<NetworkIoDirection> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: &NetworkIoDirection) -> opentelemetry::KeyValue {
        opentelemetry::KeyValue::new(self.key.clone(), v.to_string())
    }
}

/// Local address of the network connection - IP address or Unix domain socket name.
///
/// Examples:
/// - 10.1.2.80
/// - /tmp/my.sock
pub const NETWORK_LOCAL_ADDRESS: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("network.local.address");

/// Local port number of the network connection.
///
/// Examples:
/// - 65123
pub const NETWORK_LOCAL_PORT: crate::attributes::AttributeKey<i64> = crate::attributes::AttributeKey::new("network.local.port");

/// Peer address of the network connection - IP address or Unix domain socket name.
///
/// Examples:
/// - 10.1.2.80
/// - /tmp/my.sock
pub const NETWORK_PEER_ADDRESS: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("network.peer.address");

/// Peer port number of the network connection.
///
/// Examples:
/// - 65123
pub const NETWORK_PEER_PORT: crate::attributes::AttributeKey<i64> = crate::attributes::AttributeKey::new("network.peer.port");

/// [OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.
///
/// Notes:
///   The value SHOULD be normalized to lowercase.
///
/// Examples:
/// - amqp
/// - http
/// - mqtt
pub const NETWORK_PROTOCOL_NAME: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("network.protocol.name");

/// The actual version of the protocol used for network communication.
///
/// Notes:
///   If protocol version is subject to negotiation (for example using [ALPN](https://www.rfc-editor.org/rfc/rfc7301.html)), this attribute SHOULD be set to the negotiated version. If the actual protocol version is not known, this attribute SHOULD NOT be set.
///
/// Examples:
/// - 1.1
/// - 2
pub const NETWORK_PROTOCOL_VERSION: crate::attributes::AttributeKey<opentelemetry::StringValue> = crate::attributes::AttributeKey::new("network.protocol.version");

/// [OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication).
///
/// Notes:
///   The value SHOULD be normalized to lowercase.
///   
///   Consider always setting the transport when setting a port number, since
///   a port number is ambiguous without knowing the transport. For example
///   different processes could be listening on TCP port 12345 and UDP port 12345.
///
/// Examples:
/// - tcp
/// - udp
pub const NETWORK_TRANSPORT: crate::attributes::AttributeKey<NetworkTransport> = crate::attributes::AttributeKey::new("network.transport");

/// [OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NetworkTransport {    /// TCP
    Tcp,    /// UDP
    Udp,    /// Named or anonymous pipe.
    Pipe,    /// Unix domain socket
    Unix,
    /// This variant allows defining a custom entry in the enum.
    _Custom(String),
}

impl NetworkTransport {
    /// Returns the string representation of the [`NetworkTransport`].
    pub fn as_str(&self) -> &str {
        match self {
            NetworkTransport::Tcp => "tcp",
            NetworkTransport::Udp => "udp",
            NetworkTransport::Pipe => "pipe",
            NetworkTransport::Unix => "unix",
            NetworkTransport::_Custom(v) => v.as_str(),
            // Without this default case, the match expression would not
            // contain any variants if all variants are annotated with the
            // 'semconv_experimental' feature and the feature is not enabled.
            #[allow(unreachable_patterns)]
            _ => unreachable!(),
        }
    }
}

impl core::fmt::Display for NetworkTransport {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl crate::attributes::AttributeKey<NetworkTransport> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: &NetworkTransport) -> opentelemetry::KeyValue {
        opentelemetry::KeyValue::new(self.key.clone(), v.to_string())
    }
}

/// [OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.
///
/// Notes:
///   The value SHOULD be normalized to lowercase.
///
/// Examples:
/// - ipv4
/// - ipv6
pub const NETWORK_TYPE: crate::attributes::AttributeKey<NetworkType> = crate::attributes::AttributeKey::new("network.type");

/// [OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NetworkType {    /// IPv4
    Ipv4,    /// IPv6
    Ipv6,
    /// This variant allows defining a custom entry in the enum.
    _Custom(String),
}

impl NetworkType {
    /// Returns the string representation of the [`NetworkType`].
    pub fn as_str(&self) -> &str {
        match self {
            NetworkType::Ipv4 => "ipv4",
            NetworkType::Ipv6 => "ipv6",
            NetworkType::_Custom(v) => v.as_str(),
            // Without this default case, the match expression would not
            // contain any variants if all variants are annotated with the
            // 'semconv_experimental' feature and the feature is not enabled.
            #[allow(unreachable_patterns)]
            _ => unreachable!(),
        }
    }
}

impl core::fmt::Display for NetworkType {
    /// Formats the value using the given formatter.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl crate::attributes::AttributeKey<NetworkType> {
    /// Returns a [`KeyValue`] pair for the given value.
    pub fn value(&self, v: &NetworkType) -> opentelemetry::KeyValue {
        opentelemetry::KeyValue::new(self.key.clone(), v.to_string())
    }
}