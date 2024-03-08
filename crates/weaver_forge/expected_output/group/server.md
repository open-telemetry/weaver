# Group `server` (attribute_group)

## Brief

These attributes may be used to describe the server in a connection-based network interaction where there is one side that initiates the connection (the client is the side that initiates the connection). This covers all TCP network interactions since TCP is connection-based and one side initiates the connection (an exception is made for peer-to-peer communication over TCP where the "user-facing" surface of the protocol / API doesn't expose a clear notion of client and server). This also covers UDP network interactions where one side initiates the interaction, e.g. QUIC (HTTP/3) and DNS.

prefix: server

## Attributes


### Attribute `server.address`

Server domain name if available without reverse DNS lookup; otherwise, IP address or Unix domain socket name.


When observed from the client side, and when communicating through an intermediary, `server.address` SHOULD represent the server address behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Recommended

- Type: string
- Examples: [
    "example.com",
    "10.1.2.80",
    "/tmp/my.sock",
]

- Stability: Stable


### Attribute `server.port`

Server port number.


When observed from the client side, and when communicating through an intermediary, `server.port` SHOULD represent the server port behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Recommended

- Type: int
- Examples: [
    80,
    8080,
    443,
]

- Stability: Stable



## Provenance

Source: data/registry-server.yaml

