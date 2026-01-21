
# Metric Namespace `http`


## Metric `http.client.request.duration` 

Instrument: histogram
Unit: s
Stability: stable

### Attributes


#### Attribute `http.request.method`

HTTP request method.


HTTP request method value SHOULD be "known" to the instrumentation.
By default, this convention defines "known" methods as the ones listed in [RFC9110](https://www.rfc-editor.org/rfc/rfc9110.html#name-methods)
and the PATCH method defined in [RFC5789](https://www.rfc-editor.org/rfc/rfc5789.html).

If the HTTP request method is not known to instrumentation, it MUST set the `http.request.method` attribute to `_OTHER`.

If the HTTP instrumentation could end up converting valid HTTP request methods to `_OTHER`, then it MUST provide a way to override
the list of known HTTP methods. If this override is done via environment variable, then the environment variable MUST be named
OTEL_INSTRUMENTATION_HTTP_KNOWN_METHODS and support a comma-separated list of case-sensitive known HTTP methods
(this list MUST be a full override of the default known method, it is not a list of known methods in addition to the defaults).

HTTP method names are case-sensitive and `http.request.method` attribute value MUST match a known HTTP method name exactly.
Instrumentations for specific web frameworks that consider HTTP methods to be case insensitive, SHOULD populate a canonical equivalent.
Tracing instrumentations that do so, MUST also set `http.request.method_original` to the original value.

- Requirement Level: Recommended
  
- Type: Enum [CONNECT, DELETE, GET, HEAD, OPTIONS, PATCH, POST, PUT, TRACE, _OTHER]
- Examples: [
    "GET",
    "POST",
    "HEAD",
]
  
- Stability: Stable
  
  
#### Attribute `http.response.status_code`

[HTTP response status code](https://tools.ietf.org/html/rfc7231#section-6).


- Requirement Level: Recommended
  
- Type: int
- Examples: [
    200,
]
  
- Stability: Stable
  
  
#### Attribute `server.address`

Some HTTP specific description


When observed from the client side, and when communicating through an intermediary, `server.address` SHOULD represent the server address behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Required
  
- Type: string
- Examples: [
    "example.com",
    "10.1.2.80",
    "/tmp/my.sock",
]
  
- Stability: Stable
  
  
#### Attribute `server.port`

Some HTTP specific description


When observed from the client side, and when communicating through an intermediary, `server.port` SHOULD represent the server port behind any intermediaries, for example proxies, if it's available.

- Requirement Level: Required
  
- Type: int
- Examples: [
    80,
    8080,
    443,
]
  
- Stability: Stable
  
  
#### Attribute `url.scheme`

The [URI scheme](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) component identifying the used protocol.


- Requirement Level: Optional
  
- Type: string
- Examples: [
    "http",
    "https",
]
  
- Stability: Stable
  
  
  
  
# Metric Namespace `jvm`


## Metric `jvm.class.count` 

Instrument: updowncounter
Unit: {class}
Stability: stable

### Attributes


  
## Metric `jvm.class.loaded` 

Instrument: counter
Unit: {class}
Stability: stable

### Attributes


  
## Metric `jvm.class.unloaded` 

Instrument: counter
Unit: {class}
Stability: stable

### Attributes


  
## Metric `jvm.cpu.count` 

Instrument: updowncounter
Unit: {cpu}
Stability: stable

### Attributes


  
## Metric `jvm.cpu.recent_utilization` 

Instrument: gauge
Unit: 1
Stability: stable

### Attributes


  
## Metric `jvm.cpu.time` 

Instrument: counter
Unit: s
Stability: stable

### Attributes


  
## Metric `jvm.gc.duration` 

Instrument: histogram
Unit: s
Stability: stable

### Attributes


#### Attribute `jvm.gc.action`

Name of the garbage collector action.


Garbage collector action is generally obtained via [GarbageCollectionNotificationInfo#getGcAction()](https://docs.oracle.com/en/java/javase/11/docs/api/jdk.management/com/sun/management/GarbageCollectionNotificationInfo.html#getGcAction()).

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "end of minor GC",
    "end of major GC",
]
  
- Stability: Stable
  
  
#### Attribute `jvm.gc.name`

Name of the garbage collector.


Garbage collector name is generally obtained via [GarbageCollectionNotificationInfo#getGcName()](https://docs.oracle.com/en/java/javase/11/docs/api/jdk.management/com/sun/management/GarbageCollectionNotificationInfo.html#getGcName()).

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "G1 Young Generation",
    "G1 Old Generation",
]
  
- Stability: Stable
  
  
  
## Metric `jvm.memory.committed` 

Instrument: updowncounter
Unit: By
Stability: stable

### Attributes


#### Attribute `jvm.memory.pool.name`

Name of the memory pool.


Pool names are generally obtained via [MemoryPoolMXBean#getName()](https://docs.oracle.com/en/java/javase/11/docs/api/java.management/java/lang/management/MemoryPoolMXBean.html#getName()).

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "G1 Old Gen",
    "G1 Eden space",
    "G1 Survivor Space",
]
  
- Stability: Stable
  
  
#### Attribute `jvm.memory.type`

The type of memory.


- Requirement Level: Recommended
  
- Type: Enum [heap, non_heap, deprecated, experimental]
- Examples: [
    "heap",
    "non_heap",
]
  
- Stability: Stable
  
  
  
## Metric `jvm.memory.deprecated` 

Instrument: updowncounter
Unit: By
Stability: stable

### Attributes


  
## Metric `jvm.memory.limit` 

Instrument: updowncounter
Unit: By
Stability: stable

### Attributes


#### Attribute `jvm.memory.pool.name`

Name of the memory pool.


Pool names are generally obtained via [MemoryPoolMXBean#getName()](https://docs.oracle.com/en/java/javase/11/docs/api/java.management/java/lang/management/MemoryPoolMXBean.html#getName()).

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "G1 Old Gen",
    "G1 Eden space",
    "G1 Survivor Space",
]
  
- Stability: Stable
  
  
#### Attribute `jvm.memory.type`

The type of memory.


- Requirement Level: Recommended
  
- Type: Enum [heap, non_heap, deprecated, experimental]
- Examples: [
    "heap",
    "non_heap",
]
  
- Stability: Stable
  
  
  
## Metric `jvm.memory.used` 

Instrument: updowncounter
Unit: By
Stability: stable

### Attributes


#### Attribute `jvm.memory.deprecated.attribute`

Something deprecated.


- Requirement Level: Recommended
  
- Type: boolean
- Deprecated: {"note": "Use `jvm.memory.stable.attribute` instead.", "reason": "obsoleted"}
  
- Stability: Stable
  
  
#### Attribute `jvm.memory.experimental.attribute`

Something experimental.


- Requirement Level: Optional
  
- Type: boolean
  
- Stability: Development
  
  
#### Attribute `jvm.memory.pool.name`

Name of the memory pool.


Pool names are generally obtained via [MemoryPoolMXBean#getName()](https://docs.oracle.com/en/java/javase/11/docs/api/java.management/java/lang/management/MemoryPoolMXBean.html#getName()).

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "G1 Old Gen",
    "G1 Eden space",
    "G1 Survivor Space",
]
  
- Stability: Stable
  
  
#### Attribute `jvm.memory.stable.attribute`

Something stable.


- Requirement Level: Recommended
  
- Type: boolean
  
- Stability: Stable
  
  
#### Attribute `jvm.memory.type`

The type of memory.


- Requirement Level: Recommended
  
- Type: Enum [heap, non_heap, deprecated, experimental]
- Examples: [
    "heap",
    "non_heap",
]
  
- Stability: Stable
  
  
  
## Metric `jvm.memory.used_after_last_gc` 

Instrument: updowncounter
Unit: By
Stability: stable

### Attributes


#### Attribute `jvm.memory.pool.name`

Name of the memory pool.


Pool names are generally obtained via [MemoryPoolMXBean#getName()](https://docs.oracle.com/en/java/javase/11/docs/api/java.management/java/lang/management/MemoryPoolMXBean.html#getName()).

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "G1 Old Gen",
    "G1 Eden space",
    "G1 Survivor Space",
]
  
- Stability: Stable
  
  
#### Attribute `jvm.memory.type`

The type of memory.


- Requirement Level: Recommended
  
- Type: Enum [heap, non_heap, deprecated, experimental]
- Examples: [
    "heap",
    "non_heap",
]
  
- Stability: Stable
  
  
  
## Metric `jvm.thread.count` 

Instrument: updowncounter
Unit: {thread}
Stability: stable

### Attributes


#### Attribute `jvm.thread.daemon`

Whether the thread is daemon or not.


- Requirement Level: Recommended
  
- Type: boolean
  
- Stability: Stable
  
  
#### Attribute `jvm.thread.state`

State of the thread.


- Requirement Level: Recommended
  
- Type: Enum [new, runnable, blocked, waiting, timed_waiting, terminated]
- Examples: [
    "runnable",
    "blocked",
]
  
- Stability: Stable
  
  
  
  
  