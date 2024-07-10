
# Events Namespace `device.app`


## Event `device.app.lifecycle`

Note: This event identifies the fields that are common to all lifecycle events for android and iOS using the `android.state` and `ios.state` fields. The `android.state` and `ios.state` attributes are mutually exclusive.

Brief: This event represents an occurrence of a lifecycle transition on Android or iOS platform.

Requirement level: 
Stability: experimental

### Body Fields

#### Field `ios.state`

This attribute represents the state the application has transitioned into at the occurrence of the event.

The iOS lifecycle states are defined in the [UIApplicationDelegate documentation](https://developer.apple.com/documentation/uikit/uiapplicationdelegate#1656902), and from which the `OS terminology` column values are derived.

- Requirement Level: Conditionally Required - if and only if `os.name` is `ios`
- Type: Enum [active, inactive, background, foreground, terminate]
- Stability: Experimental

#### Field `android.state`

This attribute represents the state the application has transitioned into at the occurrence of the event.

The Android lifecycle states are defined in [Activity lifecycle callbacks](https://developer.android.com/guide/components/activities/activity-lifecycle#lc), and from which the `OS identifiers` are derived.

- Requirement Level: Conditionally Required - if and only if `os.name` is `android`
- Type: Enum [created, background, foreground]
- Stability: Experimental

### Attributes


  
  
# Events Namespace `gen_ai.content`


## Event `gen_ai.content.completion`

Note: 
Brief: In the lifetime of an GenAI span, events for prompts sent and completions received may be created, depending on the configuration of the instrumentation.

Requirement level: 
Stability: 

### Body Fields

No event body defined.

### Attributes


#### Attribute `gen_ai.completion`

The full response received from the GenAI model.


It's RECOMMENDED to format completions as JSON string matching [OpenAI messages format](https://platform.openai.com/docs/guides/text-generation)

- Requirement Level: Conditionally Required - if and only if corresponding event is enabled
  
- Type: string
- Examples: [
    "[{'role': 'assistant', 'content': 'The capital of France is Paris.'}]",
]
  
- Stability: Experimental
  
  
  
## Event `gen_ai.content.prompt`

Note: 
Brief: In the lifetime of an GenAI span, events for prompts sent and completions received may be created, depending on the configuration of the instrumentation.

Requirement level: 
Stability: 

### Body Fields

No event body defined.

### Attributes


#### Attribute `gen_ai.prompt`

The full prompt sent to the GenAI model.


It's RECOMMENDED to format prompts as JSON string matching [OpenAI messages format](https://platform.openai.com/docs/guides/text-generation)

- Requirement Level: Conditionally Required - if and only if corresponding event is enabled
  
- Type: string
- Examples: [
    "[{'role': 'user', 'content': 'What is the capital of France?'}]",
]
  
- Stability: Experimental
  
  
  
  
# Events Namespace `other`


## Event `feature_flag`

Note: 
Brief: This semantic convention defines the attributes used to represent a feature flag evaluation as an event.

Requirement level: 
Stability: 

### Body Fields

No event body defined.

### Attributes


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

No event body defined.

### Attributes


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
  
  
  
  
# Events Namespace `rpc`


## Event `rpc.message`

Note: 
Brief: RPC received/sent message.
Requirement level: 
Stability: 

### Body Fields

No event body defined.

### Attributes


#### Attribute `rpc.message.type`

Whether this is a received or sent message.


- Requirement Level: Recommended
  
- Type: Enum [SENT, RECEIVED]
  
- Stability: Experimental
  
  
#### Attribute `rpc.message.id`

MUST be calculated as two different counters starting from `1` one for sent messages and one for received message.


This way we guarantee that the values will be consistent between different implementations.

- Requirement Level: Recommended
  
- Type: int
  
- Stability: Experimental
  
  
#### Attribute `rpc.message.compressed_size`

Compressed size of the message in bytes.


- Requirement Level: Recommended
  
- Type: int
  
- Stability: Experimental
  
  
#### Attribute `rpc.message.uncompressed_size`

Uncompressed size of the message in bytes.


- Requirement Level: Recommended
  
- Type: int
  
- Stability: Experimental
  
  
  
  
  