# Group `registry.network` (attribute_group)

## Brief

These attributes may be used for any network related operation.

prefix: network

## Attributes


### Attribute `network.carrier.icc`

The ISO 3166-1 alpha-2 2-character country code associated with the mobile carrier network.


- Requirement Level: Recommended

- Type: string
- Examples: DE


### Attribute `network.carrier.mcc`

The mobile carrier country code.


- Requirement Level: Recommended

- Type: string
- Examples: 310


### Attribute `network.carrier.mnc`

The mobile carrier network code.


- Requirement Level: Recommended

- Type: string
- Examples: 001


### Attribute `network.carrier.name`

The name of the mobile carrier.


- Requirement Level: Recommended

- Type: string
- Examples: sprint


### Attribute `network.connection.subtype`

This describes more details regarding the connection.type. It may be the type of cell technology connection, but it could be used for describing details about a wifi connection.


- Requirement Level: Recommended

- Type: Enum [gprs, edge, umts, cdma, evdo_0, evdo_a, cdma2000_1xrtt, hsdpa, hsupa, hspa, iden, evdo_b, lte, ehrpd, hspap, gsm, td_scdma, iwlan, nr, nrnsa, lte_ca]
- Examples: LTE


### Attribute `network.connection.type`

The internet connection type.


- Requirement Level: Recommended

- Type: Enum [wifi, wired, cell, unavailable, unknown]
- Examples: wifi


### Attribute `network.local.address`

Local address of the network connection - IP address or Unix domain socket name.


- Requirement Level: Recommended

- Type: string
- Examples: [
    "10.1.2.80",
    "/tmp/my.sock",
]

- Stability: Stable


### Attribute `network.local.port`

Local port number of the network connection.


- Requirement Level: Recommended

- Type: int
- Examples: [
    65123,
]

- Stability: Stable


### Attribute `network.peer.address`

Peer address of the network connection - IP address or Unix domain socket name.


- Requirement Level: Recommended

- Type: string
- Examples: [
    "10.1.2.80",
    "/tmp/my.sock",
]

- Stability: Stable


### Attribute `network.peer.port`

Peer port number of the network connection.


- Requirement Level: Recommended

- Type: int
- Examples: [
    65123,
]

- Stability: Stable


### Attribute `network.protocol.name`

[OSI application layer](https://osi-model.com/application-layer/) or non-OSI equivalent.


The value SHOULD be normalized to lowercase.

- Requirement Level: Recommended

- Type: string
- Examples: [
    "amqp",
    "http",
    "mqtt",
]

- Stability: Stable


### Attribute `network.protocol.version`

Version of the protocol specified in `network.protocol.name`.


`network.protocol.version` refers to the version of the protocol used and might be different from the protocol client's version. If the HTTP client has a version of `0.27.2`, but sends HTTP version `1.1`, this attribute should be set to `1.1`.

- Requirement Level: Recommended

- Type: string
- Examples: 3.1.1

- Stability: Stable


### Attribute `network.transport`

[OSI transport layer](https://osi-model.com/transport-layer/) or [inter-process communication method](https://wikipedia.org/wiki/Inter-process_communication).



The value SHOULD be normalized to lowercase.

Consider always setting the transport when setting a port number, since
a port number is ambiguous without knowing the transport. For example
different processes could be listening on TCP port 12345 and UDP port 12345.

- Requirement Level: Recommended

- Type: Enum [tcp, udp, pipe, unix]
- Examples: [
    "tcp",
    "udp",
]

- Stability: Stable


### Attribute `network.type`

[OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.


The value SHOULD be normalized to lowercase.

- Requirement Level: Recommended

- Type: Enum [ipv4, ipv6]
- Examples: [
    "ipv4",
    "ipv6",
]

- Stability: Stable


### Attribute `network.io.direction`

The network IO operation direction.


- Requirement Level: Recommended

- Type: Enum [transmit, receive]
- Examples: [
    "transmit",
]



## Lineage

Source file: data/registry-network.yaml

