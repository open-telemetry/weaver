# Semantic Convention Resource Groups


## Namespace Resource `library`



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
- Deprecated: 
  
  
#### Attribute `otel.library.version`




- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "1.0.0",
]
- Deprecated: 
  
  
  
- 
## Namespace Resource `scope`



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