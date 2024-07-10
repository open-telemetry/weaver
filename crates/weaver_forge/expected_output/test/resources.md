# Semantic Convention Resource Groups


## Namespace Resource `otel`



## Resource `otel.library`

Note: 
Brief: Span attributes used by non-OTLP exporters to represent OpenTelemetry Scope's concepts.


### Attributes


#### Attribute `otel.library.name`




- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "io.opentelemetry.contrib.mongodb",
]
- Deprecated: use the `otel.scope.name` attribute.
  
  
#### Attribute `otel.library.version`




- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "1.0.0",
]
- Deprecated: use the `otel.scope.version` attribute.
  
  
  

## Resource `otel.scope`

Note: 
Brief: Attributes used by non-OTLP exporters to represent OpenTelemetry Scope's concepts.

### Attributes


#### Attribute `otel.scope.name`

The name of the instrumentation scope - (`InstrumentationScope.Name` in OTLP).


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "io.opentelemetry.contrib.mongodb",
]
  
- Stability: Stable
  
  
#### Attribute `otel.scope.version`

The version of the instrumentation scope - (`InstrumentationScope.Version` in OTLP).


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "1.0.0",
]
  
- Stability: Stable
  
  
  
- 
## Namespace Resource `other`



## Resource `browser`

Note: 
Brief: The web browser in which the application represented by the resource is running. The `browser.*` attributes MUST be used only for resources that represent applications running in a web browser (regardless of whether running on a mobile or desktop device).


### Attributes


#### Attribute `browser.brands`

Array of brand name and version separated by a space


This value is intended to be taken from the [UA client hints API](https://wicg.github.io/ua-client-hints/#interface) (`navigator.userAgentData.brands`).

- Requirement Level: Recommended
  
- Type: string[]
- Examples: [
    " Not A;Brand 99",
    "Chromium 99",
    "Chrome 99",
]
  
  
#### Attribute `browser.platform`

The platform on which the browser is running


This value is intended to be taken from the [UA client hints API](https://wicg.github.io/ua-client-hints/#interface) (`navigator.userAgentData.platform`). If unavailable, the legacy `navigator.platform` API SHOULD NOT be used instead and this attribute SHOULD be left unset in order for the values to be consistent.
The list of possible values is defined in the [W3C User-Agent Client Hints specification](https://wicg.github.io/ua-client-hints/#sec-ch-ua-platform). Note that some (but not all) of these values can overlap with values in the [`os.type` and `os.name` attributes](./os.md). However, for consistency, the values in the `browser.platform` attribute should capture the exact value that the user agent provides.

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "Windows",
    "macOS",
    "Android",
]
  
  
#### Attribute `browser.mobile`

A boolean that is true if the browser is running on a mobile device


This value is intended to be taken from the [UA client hints API](https://wicg.github.io/ua-client-hints/#interface) (`navigator.userAgentData.mobile`). If unavailable, this attribute SHOULD be left unset.

- Requirement Level: Recommended
  
- Type: boolean
  
  
#### Attribute `browser.language`

Preferred language of the user using the browser


This value is intended to be taken from the Navigator API `navigator.language`.

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "en",
    "en-US",
    "fr",
    "fr-FR",
]
  
  
#### Attribute `user_agent.original`

Full user-agent string provided by the browser


The user-agent value SHOULD be provided only from browsers that do not have a mechanism to retrieve brands and platform individually from the User-Agent Client Hints API. To retrieve the value, the legacy `navigator.userAgent` API can be used.

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/95.0.4638.54 Safari/537.36",
]
  
- Stability: Stable
  
  
  
- 