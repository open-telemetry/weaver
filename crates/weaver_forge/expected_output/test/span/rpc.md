## Namespace Span `rpc`


## Span `rpc.client`

This document defines semantic conventions for remote procedure call client spans.

Prefix: 
Kind: none

### Attributes


#### Attribute `network.peer.address`

Peer address of the network connection - IP address or Unix domain socket name.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  

#### Attribute `network.transport`

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
  

#### Attribute `network.type`

[OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.


The value SHOULD be normalized to lowercase.

- Requirement Level: Recommended
  
- Type: Enum [ipv4, ipv6]
- Examples: [
    "ipv4",
    "ipv6",
]
  
- Stability: Stable
  

#### Attribute `rpc.method`

The name of the (logical) method being called, must be equal to the $method part in the span name.


This is the logical name of the method from the RPC interface perspective, which can be different from the name of any implementing method/function. The `code.function` attribute may be used to store the latter (e.g., method actually executing the call on the server side, RPC client stub method on the client side).

- Requirement Level: Recommended
  
- Type: string
- Examples: exampleMethod
  
- Stability: Experimental
  

#### Attribute `rpc.service`

The full (logical) name of the service being called, including its package name, if applicable.


This is the logical name of the service from the RPC interface perspective, which can be different from the name of any implementing class. The `code.namespace` attribute may be used to store the latter (despite the attribute name, it may include a class name; e.g., class with method actually executing the call on the server side, RPC client stub class on the client side).

- Requirement Level: Recommended
  
- Type: string
- Examples: myservice.EchoService
  
- Stability: Experimental
  

#### Attribute `rpc.system`

A string identifying the remoting system. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Type: Enum [grpc, java_rmi, dotnet_wcf, apache_dubbo, connect_rpc]
  
- Stability: Experimental
  

#### Attribute `server.address`

RPC server [host name](https://grpc.github.io/grpc/core/md_doc_naming.html).



May contain server IP address, DNS name, or local socket name. When host component is an IP address, instrumentations SHOULD NOT do a reverse proxy lookup to obtain DNS name and SHOULD set `server.address` to the IP address provided in the host component.

- Requirement Level: Required
  
- Type: string
- Examples: [
    "example.com",
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  

#### Attribute `server.port`

Server port number.


When observed from the client side, and when communicating through an intermediary, `server.port` SHOULD represent the server port behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Conditionally Required - if the port is supported by the network transport used for communication.
  
- Type: int
- Examples: [
    80,
    8080,
    443,
]
  
- Stability: Stable
  

#### Attribute `network.peer.port`

Peer port number of the network connection.


- Requirement Level: Optional
  
- Type: int
- Examples: [
    65123,
]
  
- Stability: Stable
  


## Span `rpc.connect_rpc`

Tech-specific attributes for Connect RPC.

Prefix: 
Kind: none

### Attributes


#### Attribute `network.transport`

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
  

#### Attribute `network.type`

[OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.


The value SHOULD be normalized to lowercase.

- Requirement Level: Recommended
  
- Type: Enum [ipv4, ipv6]
- Examples: [
    "ipv4",
    "ipv6",
]
  
- Stability: Stable
  

#### Attribute `rpc.method`

The name of the (logical) method being called, must be equal to the $method part in the span name.


This is the logical name of the method from the RPC interface perspective, which can be different from the name of any implementing method/function. The `code.function` attribute may be used to store the latter (e.g., method actually executing the call on the server side, RPC client stub method on the client side).

- Requirement Level: Recommended
  
- Type: string
- Examples: exampleMethod
  
- Stability: Experimental
  

#### Attribute `rpc.service`

The full (logical) name of the service being called, including its package name, if applicable.


This is the logical name of the service from the RPC interface perspective, which can be different from the name of any implementing class. The `code.namespace` attribute may be used to store the latter (despite the attribute name, it may include a class name; e.g., class with method actually executing the call on the server side, RPC client stub class on the client side).

- Requirement Level: Recommended
  
- Type: string
- Examples: myservice.EchoService
  
- Stability: Experimental
  

#### Attribute `rpc.system`

A string identifying the remoting system. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Type: Enum [grpc, java_rmi, dotnet_wcf, apache_dubbo, connect_rpc]
  
- Stability: Experimental
  

#### Attribute `server.address`

RPC server [host name](https://grpc.github.io/grpc/core/md_doc_naming.html).



May contain server IP address, DNS name, or local socket name. When host component is an IP address, instrumentations SHOULD NOT do a reverse proxy lookup to obtain DNS name and SHOULD set `server.address` to the IP address provided in the host component.

- Requirement Level: Required
  
- Type: string
- Examples: [
    "example.com",
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  

#### Attribute `server.port`

Server port number.


When observed from the client side, and when communicating through an intermediary, `server.port` SHOULD represent the server port behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Conditionally Required - if the port is supported by the network transport used for communication.
  
- Type: int
- Examples: [
    80,
    8080,
    443,
]
  
- Stability: Stable
  

#### Attribute `rpc.connect_rpc.error_code`

The [error codes](https://connect.build/docs/protocol/#error-codes) of the Connect request. Error codes are always string values.


- Requirement Level: Conditionally Required - If response is not successful and if error code available.
  
- Tag: connect_rpc-tech-specific
  
- Type: Enum [cancelled, unknown, invalid_argument, deadline_exceeded, not_found, already_exists, permission_denied, resource_exhausted, failed_precondition, aborted, out_of_range, unimplemented, internal, unavailable, data_loss, unauthenticated]
  
- Stability: Experimental
  

#### Attribute `rpc.connect_rpc.request.metadata`

Connect request metadata, `<key>` being the normalized Connect Metadata key (lowercase), the value being the metadata values.



Instrumentations SHOULD require an explicit configuration of which metadata values are to be captured. Including all request metadata values can be a security risk - explicit configuration helps avoid leaking sensitive information.

- Requirement Level: Optional
  
- Tag: connect_rpc-tech-specific
  
- Type: template[string[]]
- Examples: [
    "rpc.request.metadata.my-custom-metadata-attribute=[\"1.2.3.4\", \"1.2.3.5\"]",
]
  
- Stability: Experimental
  

#### Attribute `rpc.connect_rpc.response.metadata`

Connect response metadata, `<key>` being the normalized Connect Metadata key (lowercase), the value being the metadata values.



Instrumentations SHOULD require an explicit configuration of which metadata values are to be captured. Including all response metadata values can be a security risk - explicit configuration helps avoid leaking sensitive information.

- Requirement Level: Optional
  
- Tag: connect_rpc-tech-specific
  
- Type: template[string[]]
- Examples: [
    "rpc.response.metadata.my-custom-metadata-attribute=[\"attribute_value\"]",
]
  
- Stability: Experimental
  


## Span `rpc.grpc`

Tech-specific attributes for gRPC.

Prefix: 
Kind: none

### Attributes


#### Attribute `network.transport`

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
  

#### Attribute `network.type`

[OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.


The value SHOULD be normalized to lowercase.

- Requirement Level: Recommended
  
- Type: Enum [ipv4, ipv6]
- Examples: [
    "ipv4",
    "ipv6",
]
  
- Stability: Stable
  

#### Attribute `rpc.method`

The name of the (logical) method being called, must be equal to the $method part in the span name.


This is the logical name of the method from the RPC interface perspective, which can be different from the name of any implementing method/function. The `code.function` attribute may be used to store the latter (e.g., method actually executing the call on the server side, RPC client stub method on the client side).

- Requirement Level: Recommended
  
- Type: string
- Examples: exampleMethod
  
- Stability: Experimental
  

#### Attribute `rpc.service`

The full (logical) name of the service being called, including its package name, if applicable.


This is the logical name of the service from the RPC interface perspective, which can be different from the name of any implementing class. The `code.namespace` attribute may be used to store the latter (despite the attribute name, it may include a class name; e.g., class with method actually executing the call on the server side, RPC client stub class on the client side).

- Requirement Level: Recommended
  
- Type: string
- Examples: myservice.EchoService
  
- Stability: Experimental
  

#### Attribute `rpc.system`

A string identifying the remoting system. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Type: Enum [grpc, java_rmi, dotnet_wcf, apache_dubbo, connect_rpc]
  
- Stability: Experimental
  

#### Attribute `server.address`

RPC server [host name](https://grpc.github.io/grpc/core/md_doc_naming.html).



May contain server IP address, DNS name, or local socket name. When host component is an IP address, instrumentations SHOULD NOT do a reverse proxy lookup to obtain DNS name and SHOULD set `server.address` to the IP address provided in the host component.

- Requirement Level: Required
  
- Type: string
- Examples: [
    "example.com",
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  

#### Attribute `server.port`

Server port number.


When observed from the client side, and when communicating through an intermediary, `server.port` SHOULD represent the server port behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Conditionally Required - if the port is supported by the network transport used for communication.
  
- Type: int
- Examples: [
    80,
    8080,
    443,
]
  
- Stability: Stable
  

#### Attribute `rpc.grpc.request.metadata`

gRPC request metadata, `<key>` being the normalized gRPC Metadata key (lowercase), the value being the metadata values.



Instrumentations SHOULD require an explicit configuration of which metadata values are to be captured. Including all request metadata values can be a security risk - explicit configuration helps avoid leaking sensitive information.

- Requirement Level: Optional
  
- Tag: grpc-tech-specific
  
- Type: template[string[]]
- Examples: [
    "rpc.grpc.request.metadata.my-custom-metadata-attribute=[\"1.2.3.4\", \"1.2.3.5\"]",
]
  
- Stability: Experimental
  

#### Attribute `rpc.grpc.response.metadata`

gRPC response metadata, `<key>` being the normalized gRPC Metadata key (lowercase), the value being the metadata values.



Instrumentations SHOULD require an explicit configuration of which metadata values are to be captured. Including all response metadata values can be a security risk - explicit configuration helps avoid leaking sensitive information.

- Requirement Level: Optional
  
- Tag: grpc-tech-specific
  
- Type: template[string[]]
- Examples: [
    "rpc.grpc.response.metadata.my-custom-metadata-attribute=[\"attribute_value\"]",
]
  
- Stability: Experimental
  

#### Attribute `rpc.grpc.status_code`

The [numeric status code](https://github.com/grpc/grpc/blob/v1.33.2/doc/statuscodes.md) of the gRPC request.


- Requirement Level: Required
  
- Tag: grpc-tech-specific
  
- Type: Enum [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
  
- Stability: Experimental
  


## Span `rpc.jsonrpc`

Tech-specific attributes for [JSON RPC](https://www.jsonrpc.org/).

Prefix: rpc.jsonrpc
Kind: none

### Attributes


#### Attribute `network.transport`

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
  

#### Attribute `network.type`

[OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.


The value SHOULD be normalized to lowercase.

- Requirement Level: Recommended
  
- Type: Enum [ipv4, ipv6]
- Examples: [
    "ipv4",
    "ipv6",
]
  
- Stability: Stable
  

#### Attribute `rpc.service`

The full (logical) name of the service being called, including its package name, if applicable.


This is the logical name of the service from the RPC interface perspective, which can be different from the name of any implementing class. The `code.namespace` attribute may be used to store the latter (despite the attribute name, it may include a class name; e.g., class with method actually executing the call on the server side, RPC client stub class on the client side).

- Requirement Level: Recommended
  
- Type: string
- Examples: myservice.EchoService
  
- Stability: Experimental
  

#### Attribute `rpc.system`

A string identifying the remoting system. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Type: Enum [grpc, java_rmi, dotnet_wcf, apache_dubbo, connect_rpc]
  
- Stability: Experimental
  

#### Attribute `server.address`

RPC server [host name](https://grpc.github.io/grpc/core/md_doc_naming.html).



May contain server IP address, DNS name, or local socket name. When host component is an IP address, instrumentations SHOULD NOT do a reverse proxy lookup to obtain DNS name and SHOULD set `server.address` to the IP address provided in the host component.

- Requirement Level: Required
  
- Type: string
- Examples: [
    "example.com",
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  

#### Attribute `server.port`

Server port number.


When observed from the client side, and when communicating through an intermediary, `server.port` SHOULD represent the server port behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Conditionally Required - if the port is supported by the network transport used for communication.
  
- Type: int
- Examples: [
    80,
    8080,
    443,
]
  
- Stability: Stable
  

#### Attribute `rpc.jsonrpc.error_code`

`error.code` property of response if it is an error response.


- Requirement Level: Conditionally Required - If response is not successful.
  
- Tag: jsonrpc-tech-specific
  
- Type: int
- Examples: [
    -32700,
    100,
]
  
- Stability: Experimental
  

#### Attribute `rpc.jsonrpc.error_message`

`error.message` property of response if it is an error response.


- Requirement Level: Recommended
  
- Tag: jsonrpc-tech-specific
  
- Type: string
- Examples: [
    "Parse error",
    "User already exists",
]
  
- Stability: Experimental
  

#### Attribute `rpc.jsonrpc.request_id`

`id` property of request or response. Since protocol allows id to be int, string, `null` or missing (for notifications), value is expected to be cast to string for simplicity. Use empty string in case of `null` value. Omit entirely if this is a notification.



- Requirement Level: Recommended
  
- Tag: jsonrpc-tech-specific
  
- Type: string
- Examples: [
    "10",
    "request-7",
    "",
]
  
- Stability: Experimental
  

#### Attribute `rpc.jsonrpc.version`

Protocol version as in `jsonrpc` property of request/response. Since JSON-RPC 1.0 doesn't specify this, the value can be omitted.


- Requirement Level: Conditionally Required - If other than the default version (`1.0`)
  
- Tag: jsonrpc-tech-specific
  
- Type: string
- Examples: [
    "2.0",
    "1.0",
]
  
- Stability: Experimental
  

#### Attribute `rpc.method`

The name of the (logical) method being called, must be equal to the $method part in the span name.


This is always required for jsonrpc. See the note in the general RPC conventions for more information.

- Requirement Level: Required
  
- Tag: jsonrpc-tech-specific
  
- Type: string
- Examples: exampleMethod
  
- Stability: Experimental
  


## Span `rpc.server`

Semantic Convention for RPC server spans

Prefix: 
Kind: server

### Attributes


#### Attribute `client.address`

Client address - domain name if available without reverse DNS lookup; otherwise, IP address or Unix domain socket name.


When observed from the server side, and when communicating through an intermediary, `client.address` SHOULD represent the client address behind any intermediaries,  for example proxies, if it's available.

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "client.example.com",
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  

#### Attribute `client.port`

Client port number.


When observed from the server side, and when communicating through an intermediary, `client.port` SHOULD represent the client port behind any intermediaries,  for example proxies, if it's available.

- Requirement Level: Recommended
  
- Type: int
- Examples: [
    65123,
]
  
- Stability: Stable
  

#### Attribute `network.peer.address`

Peer address of the network connection - IP address or Unix domain socket name.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  

#### Attribute `network.transport`

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
  

#### Attribute `network.type`

[OSI network layer](https://osi-model.com/network-layer/) or non-OSI equivalent.


The value SHOULD be normalized to lowercase.

- Requirement Level: Recommended
  
- Type: Enum [ipv4, ipv6]
- Examples: [
    "ipv4",
    "ipv6",
]
  
- Stability: Stable
  

#### Attribute `rpc.method`

The name of the (logical) method being called, must be equal to the $method part in the span name.


This is the logical name of the method from the RPC interface perspective, which can be different from the name of any implementing method/function. The `code.function` attribute may be used to store the latter (e.g., method actually executing the call on the server side, RPC client stub method on the client side).

- Requirement Level: Recommended
  
- Type: string
- Examples: exampleMethod
  
- Stability: Experimental
  

#### Attribute `rpc.service`

The full (logical) name of the service being called, including its package name, if applicable.


This is the logical name of the service from the RPC interface perspective, which can be different from the name of any implementing class. The `code.namespace` attribute may be used to store the latter (despite the attribute name, it may include a class name; e.g., class with method actually executing the call on the server side, RPC client stub class on the client side).

- Requirement Level: Recommended
  
- Type: string
- Examples: myservice.EchoService
  
- Stability: Experimental
  

#### Attribute `rpc.system`

A string identifying the remoting system. See below for a list of well-known identifiers.


- Requirement Level: Required
  
- Type: Enum [grpc, java_rmi, dotnet_wcf, apache_dubbo, connect_rpc]
  
- Stability: Experimental
  

#### Attribute `server.address`

RPC server [host name](https://grpc.github.io/grpc/core/md_doc_naming.html).



May contain server IP address, DNS name, or local socket name. When host component is an IP address, instrumentations SHOULD NOT do a reverse proxy lookup to obtain DNS name and SHOULD set `server.address` to the IP address provided in the host component.

- Requirement Level: Required
  
- Type: string
- Examples: [
    "example.com",
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  

#### Attribute `server.port`

Server port number.


When observed from the client side, and when communicating through an intermediary, `server.port` SHOULD represent the server port behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Conditionally Required - if the port is supported by the network transport used for communication.
  
- Type: int
- Examples: [
    80,
    8080,
    443,
]
  
- Stability: Stable
  

#### Attribute `network.peer.port`

Peer port number of the network connection.


- Requirement Level: Optional
  
- Type: int
- Examples: [
    65123,
]
  
- Stability: Stable
  

 