# Attributes: `network`

This document describes the `network` attributes.

## `network.carrier.icc`

The ISO 3166-1 alpha-2 2-character country code associated with the mobile carrier network.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `DE` |

## `network.carrier.mcc`

The mobile carrier country code.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `310` |

## `network.carrier.mnc`

The mobile carrier network code.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `001` |

## `network.carrier.name`

The name of the mobile carrier.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `sprint` |

## `network.connection.subtype`

This describes more details regarding the connection.type. It may be the type of cell technology connection, but it could be used for describing details about a wifi connection.

| Property | Value |
|----------|-------|
| Type | Enum: `gprs`, `edge`, `umts`, `cdma`, `evdo_0`, `evdo_a`, `cdma2000_1xrtt`, `hsdpa`, `hsupa`, `hspa`, `iden`, `evdo_b`, `lte`, `ehrpd`, `hspap`, `gsm`, `td_scdma`, `iwlan`, `nr`, `nrnsa`, `lte_ca`|
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `LTE` |

### Enum Values

| Value | Description | Stability |
|-------|-------------|-----------|
| `gprs` | GPRS | Stable |
| `edge` | EDGE | Stable |
| `umts` | UMTS | Stable |
| `cdma` | CDMA | Stable |
| `evdo_0` | EVDO Rel. 0 | Stable |
| `evdo_a` | EVDO Rev. A | Stable |
| `cdma2000_1xrtt` | CDMA2000 1XRTT | Stable |
| `hsdpa` | HSDPA | Stable |
| `hsupa` | HSUPA | Stable |
| `hspa` | HSPA | Stable |
| `iden` | IDEN | Stable |
| `evdo_b` | EVDO Rev. B | Stable |
| `lte` | LTE | Stable |
| `ehrpd` | EHRPD | Stable |
| `hspap` | HSPAP | Stable |
| `gsm` | GSM | Stable |
| `td_scdma` | TD-SCDMA | Stable |
| `iwlan` | IWLAN | Stable |
| `nr` | 5G NR (New Radio) | Stable |
| `nrnsa` | 5G NRNSA (New Radio Non-Standalone) | Stable |
| `lte_ca` | LTE CA | Stable |

## `network.connection.type`

The internet connection type.

| Property | Value |
|----------|-------|
| Type | Enum: `wifi`, `wired`, `cell`, `unavailable`, `unknown`|
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `wifi` |

### Enum Values

| Value | Description | Stability |
|-------|-------------|-----------|
| `wifi` | - | Stable |
| `wired` | - | Stable |
| `cell` | - | Stable |
| `unavailable` | - | Stable |
| `unknown` | - | Stable |

## `network.io.direction`

The network IO operation direction.

| Property | Value |
|----------|-------|
| Type | Enum: `transmit`, `receive`|
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `transmit` |

### Enum Values

| Value | Description | Stability |
|-------|-------------|-----------|
| `transmit` | - | Stable |
| `receive` | - | Stable |

## `network.local.address`

Local address of the network connection - IP address or Unix domain socket name.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `10.1.2.80`, `/tmp/my.sock` |

## `network.local.port`

Local port number of the network connection.

| Property | Value |
|----------|-------|
| Type | `int` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `65123` |

## `network.peer.address`

Peer address of the network connection - IP address or Unix domain socket name.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `10.1.2.80`, `/tmp/my.sock` |

## `network.peer.port`

Peer port number of the network connection.

| Property | Value |
|----------|-------|
| Type | `int` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `65123` |

## `network.protocol.name`

[OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.

The value SHOULD be normalized to lowercase.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `amqp`, `http`, `mqtt` |

## `network.protocol.version`

Version of the protocol specified in `network.protocol.name`.

`network.protocol.version` refers to the version of the protocol used and might be different from the protocol client's version. If the HTTP client has a version of `0.27.2`, but sends HTTP version `1.1`, this attribute should be set to `1.1`.

| Property | Value |
|----------|-------|
| Type | `string` |
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `3.1.1` |

## `network.transport`

[OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication).

The value SHOULD be normalized to lowercase.

Consider always setting the transport when setting a port number, since
a port number is ambiguous without knowing the transport. For example
different processes could be listening on TCP port 12345 and UDP port 12345.

| Property | Value |
|----------|-------|
| Type | Enum: `tcp`, `udp`, `pipe`, `unix`|
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `tcp`, `udp` |

### Enum Values

| Value | Description | Stability |
|-------|-------------|-----------|
| `tcp` | TCP | Stable |
| `udp` | UDP | Stable |
| `pipe` | Named or anonymous pipe. | Stable |
| `unix` | Unix domain socket | Stable |

## `network.type`

[OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.

The value SHOULD be normalized to lowercase.

| Property | Value |
|----------|-------|
| Type | Enum: `ipv4`, `ipv6`|
| Requirement Level | Recommended |
| Stability | Stable |
| Examples | `ipv4`, `ipv6` |

### Enum Values

| Value | Description | Stability |
|-------|-------------|-----------|
| `ipv4` | IPv4 | Stable |
| `ipv6` | IPv6 | Stable |

