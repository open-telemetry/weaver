## Metrics Namespace `jvm` 


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
  
  
  