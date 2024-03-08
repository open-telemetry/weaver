## Group `registry.url` (attribute_group)

### Brief

Attributes describing URL.

prefix: url

### Attributes


#### Attribute `url.scheme`

The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "https",
    "ftp",
    "telnet",
]
  
- Stability: Stable
  
  
#### Attribute `url.full`

Absolute URL describing a network resource according to [RFC3986](https://www.rfc-editor.org/rfc/rfc3986)


For network calls, URL usually has `scheme://host[:port][path][?query][#fragment]` format, where the fragment is not transmitted over HTTP, but if it is known, it SHOULD be included nevertheless.
`url.full` MUST NOT contain credentials passed via URL in form of `https://username:password@www.example.com/`. In such case username and password SHOULD be redacted and attribute's value SHOULD be `https://REDACTED:REDACTED@www.example.com/`.
`url.full` SHOULD capture the absolute URL when it is available (or can be reconstructed) and SHOULD NOT be validated or modified except for sanitizing purposes.

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "https://www.foo.bar/search?q=OpenTelemetry#SemConv",
    "//localhost",
]
  
- Stability: Stable
  
  
#### Attribute `url.path`

The [URI path](https://www.rfc-editor.org/rfc/rfc3986#section-3.3) component


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "/search",
]
  
- Stability: Stable
  
  
#### Attribute `url.query`

The [URI query](https://www.rfc-editor.org/rfc/rfc3986#section-3.4) component


Sensitive content provided in query string SHOULD be scrubbed when instrumentations can identify it.

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "q=OpenTelemetry",
]
  
- Stability: Stable
  
  
#### Attribute `url.fragment`

The [URI fragment](https://www.rfc-editor.org/rfc/rfc3986#section-3.5) component


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "SemConv",
]
  
- Stability: Stable
  
  