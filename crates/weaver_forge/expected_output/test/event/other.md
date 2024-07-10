## Events Namespace `other`


## Event `feature_flag`

Note: 
Brief: This semantic convention defines the attributes used to represent a feature flag evaluation as an event.

Requirement level: 
Stability: 

### Body Fields

No event body defined.### Attributes


#### Attribute `feature_flag.key`

The unique identifier of the feature flag.


- Requirement Level: Required
  
- Type: string
- Examples: [
    "logo-color",
]
  
  
#### Attribute `feature_flag.provider_name`

The name of the service provider that performs the flag evaluation.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "Flag Manager",
]
  
  
#### Attribute `feature_flag.variant`

SHOULD be a semantic identifier for a value. If one is unavailable, a stringified version of the value can be used.



A semantic identifier, commonly referred to as a variant, provides a means
for referring to a value without including the value itself. This can
provide additional context for understanding the meaning behind a value.
For example, the variant `red` maybe be used for the value `#c05543`.

A stringified version of the value can be used in situations where a
semantic identifier is unavailable. String representation of the value
should be determined by the implementer.

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "red",
    "true",
    "on",
]
  
  
  
## Event `trace-exception`

Note: 
Brief: This document defines the attributes used to report a single exception associated with a span.

Requirement level: 
Stability: 

### Body Fields

No event body defined.### Attributes


#### Attribute `exception.stacktrace`

A stacktrace as a string in the natural representation for the language runtime. The representation is to be determined and documented by each language SIG.



- Requirement Level: Recommended
  
- Type: string
- Examples: Exception in thread "main" java.lang.RuntimeException: Test exception\n at com.example.GenerateTrace.methodB(GenerateTrace.java:13)\n at com.example.GenerateTrace.methodA(GenerateTrace.java:9)\n at com.example.GenerateTrace.main(GenerateTrace.java:5)
  
- Stability: Stable
  
  
#### Attribute `exception.escaped`

SHOULD be set to true if the exception event is recorded at a point where it is known that the exception is escaping the scope of the span.



An exception is considered to have escaped (or left) the scope of a span,
if that span is ended while the exception is still logically "in flight".
This may be actually "in flight" in some languages (e.g. if the exception
is passed to a Context manager's `__exit__` method in Python) but will
usually be caught at the point of recording the exception in most languages.

It is usually not possible to determine at the point where an exception is thrown
whether it will escape the scope of a span.
However, it is trivial to know that an exception
will escape, if one checks for an active exception just before ending the span,
as done in the [example for recording span exceptions](https://opentelemetry.io/docs/specs/semconv/exceptions/exceptions-spans/#recording-an-exception).

It follows that an exception may still escape the scope of the span
even if the `exception.escaped` attribute was not set or set to false,
since the event might have been recorded at a time where it was not
clear whether the exception will escape.

- Requirement Level: Recommended
  
- Type: boolean
  
- Stability: Stable
  
  
#### Attribute `exception.type`

The type of the exception (its fully-qualified class name, if applicable). The dynamic type of the exception should be preferred over the static type in languages that support it.



- Requirement Level: Conditionally Required - Required if `exception.message` is not set, recommended otherwise.
  
- Type: string
- Examples: [
    "java.net.ConnectException",
    "OSError",
]
  
- Stability: Stable
  
  
#### Attribute `exception.message`

The exception message.


- Requirement Level: Conditionally Required - Required if `exception.type` is not set, recommended otherwise.
  
- Type: string
- Examples: [
    "Division by zero",
    "Can't convert 'int' object to str implicitly",
]
  
- Stability: Stable
  
  
  