## Namespace `rpc`

### Attributes


#### Attribute `rpc.connect_rpc.error_code`

The [error codes](https://connect.build/docs/protocol/#error-codes) of the Connect request. Error codes are always string values.


- Requirement Level: Recommended
  
- Type: Enum [cancelled, unknown, invalid_argument, deadline_exceeded, not_found, already_exists, permission_denied, resource_exhausted, failed_precondition, aborted, out_of_range, unimplemented, internal, unavailable, data_loss, unauthenticated]
  
- Stability: Experimental
  
  
#### Attribute `rpc.connect_rpc.request.metadata`

Connect request metadata, `<key>` being the normalized Connect Metadata key (lowercase), the value being the metadata values.


Instrumentations SHOULD require an explicit configuration of which metadata values are to be captured. Including all request metadata values can be a security risk - explicit configuration helps avoid leaking sensitive information.

- Requirement Level: Recommended
  
- Type: template[string[]]
- Examples: [
    "rpc.request.metadata.my-custom-metadata-attribute=[\"1.2.3.4\", \"1.2.3.5\"]",
]
  
- Stability: Experimental
  
  
#### Attribute `rpc.connect_rpc.response.metadata`

Connect response metadata, `<key>` being the normalized Connect Metadata key (lowercase), the value being the metadata values.


Instrumentations SHOULD require an explicit configuration of which metadata values are to be captured. Including all response metadata values can be a security risk - explicit configuration helps avoid leaking sensitive information.

- Requirement Level: Recommended
  
- Type: template[string[]]
- Examples: [
    "rpc.response.metadata.my-custom-metadata-attribute=[\"attribute_value\"]",
]
  
- Stability: Experimental
  
  
#### Attribute `rpc.grpc.request.metadata`

gRPC request metadata, `<key>` being the normalized gRPC Metadata key (lowercase), the value being the metadata values.


Instrumentations SHOULD require an explicit configuration of which metadata values are to be captured. Including all request metadata values can be a security risk - explicit configuration helps avoid leaking sensitive information.

- Requirement Level: Recommended
  
- Type: template[string[]]
- Examples: [
    "rpc.grpc.request.metadata.my-custom-metadata-attribute=[\"1.2.3.4\", \"1.2.3.5\"]",
]
  
- Stability: Experimental
  
  
#### Attribute `rpc.grpc.response.metadata`

gRPC response metadata, `<key>` being the normalized gRPC Metadata key (lowercase), the value being the metadata values.


Instrumentations SHOULD require an explicit configuration of which metadata values are to be captured. Including all response metadata values can be a security risk - explicit configuration helps avoid leaking sensitive information.

- Requirement Level: Recommended
  
- Type: template[string[]]
- Examples: [
    "rpc.grpc.response.metadata.my-custom-metadata-attribute=[\"attribute_value\"]",
]
  
- Stability: Experimental
  
  
#### Attribute `rpc.grpc.status_code`

The [numeric status code](https://github.com/grpc/grpc/blob/v1.33.2/doc/statuscodes.md) of the gRPC request.


- Requirement Level: Recommended
  
- Type: Enum [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
  
- Stability: Experimental
  
  
#### Attribute `rpc.jsonrpc.error_code`

`error.code` property of response if it is an error response.


- Requirement Level: Recommended
  
- Type: int
- Examples: [
    -32700,
    100,
]
  
- Stability: Experimental
  
  
#### Attribute `rpc.jsonrpc.error_message`

`error.message` property of response if it is an error response.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "Parse error",
    "User already exists",
]
  
- Stability: Experimental
  
  
#### Attribute `rpc.jsonrpc.request_id`

`id` property of request or response. Since protocol allows id to be int, string, `null` or missing (for notifications), value is expected to be cast to string for simplicity. Use empty string in case of `null` value. Omit entirely if this is a notification.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "10",
    "request-7",
    "",
]
  
- Stability: Experimental
  
  
#### Attribute `rpc.jsonrpc.version`

Protocol version as in `jsonrpc` property of request/response. Since JSON-RPC 1.0 doesn't specify this, the value can be omitted.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "2.0",
    "1.0",
]
  
- Stability: Experimental
  
  
#### Attribute `rpc.message.compressed_size`

Compressed size of the message in bytes.


- Requirement Level: Recommended
  
- Type: int
  
- Stability: Experimental
  
  
#### Attribute `rpc.message.id`

MUST be calculated as two different counters starting from `1` one for sent messages and one for received message.


This way we guarantee that the values will be consistent between different implementations.

- Requirement Level: Recommended
  
- Type: int
  
- Stability: Experimental
  
  
#### Attribute `rpc.message.type`

Whether this is a received or sent message.


- Requirement Level: Recommended
  
- Type: Enum [SENT, RECEIVED]
  
- Stability: Experimental
  
  
#### Attribute `rpc.message.uncompressed_size`

Uncompressed size of the message in bytes.


- Requirement Level: Recommended
  
- Type: int
  
- Stability: Experimental
  
  
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


- Requirement Level: Recommended
  
- Type: Enum [grpc, java_rmi, dotnet_wcf, apache_dubbo, connect_rpc]
  
- Stability: Experimental
  
  