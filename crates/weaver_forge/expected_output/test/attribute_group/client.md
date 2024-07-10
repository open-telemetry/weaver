## Namespace `client`

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
  
  