# Semantic Convention Metric Groups


## Group `metric.jvm.memory.used` (metric)

### Brief

Measure of memory used.



Prefix: 
Metric: jvm.memory.used
Instrument: updowncounter
Unit: By
Stability: Stable

### Attributes


#### Attribute `pool.name`

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
  
  
#### Attribute `type`

The type of memory.


- Requirement Level: Recommended
  
- Type: Enum [heap, non_heap]
- Examples: [
    "heap",
    "non_heap",
]
  
- Stability: Stable
  
  
  
## Group `metric.jvm.memory.committed` (metric)

### Brief

Measure of memory committed.



Prefix: 
Metric: jvm.memory.committed
Instrument: updowncounter
Unit: By
Stability: Stable

### Attributes


#### Attribute `pool.name`

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
  
  
#### Attribute `type`

The type of memory.


- Requirement Level: Recommended
  
- Type: Enum [heap, non_heap]
- Examples: [
    "heap",
    "non_heap",
]
  
- Stability: Stable
  
  
  
## Group `metric.jvm.memory.limit` (metric)

### Brief

Measure of max obtainable memory.



Prefix: 
Metric: jvm.memory.limit
Instrument: updowncounter
Unit: By
Stability: Stable

### Attributes


#### Attribute `pool.name`

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
  
  
#### Attribute `type`

The type of memory.


- Requirement Level: Recommended
  
- Type: Enum [heap, non_heap]
- Examples: [
    "heap",
    "non_heap",
]
  
- Stability: Stable
  
  
  
## Group `metric.jvm.memory.used_after_last_gc` (metric)

### Brief

Measure of memory used, as measured after the most recent garbage collection event on this pool.



Prefix: 
Metric: jvm.memory.used_after_last_gc
Instrument: updowncounter
Unit: By
Stability: Stable

### Attributes


#### Attribute `pool.name`

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
  
  
#### Attribute `type`

The type of memory.


- Requirement Level: Recommended
  
- Type: Enum [heap, non_heap]
- Examples: [
    "heap",
    "non_heap",
]
  
- Stability: Stable
  
  
  
## Group `metric.jvm.gc.duration` (metric)

### Brief

Duration of JVM garbage collection actions.



Prefix: jvm.gc
Metric: jvm.gc.duration
Instrument: histogram
Unit: s
Stability: Stable

### Attributes


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
  
  
  
## Group `metric.jvm.thread.count` (metric)

### Brief

Number of executing platform threads.



Prefix: 
Metric: jvm.thread.count
Instrument: updowncounter
Unit: {thread}
Stability: Stable

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
  
  
  
## Group `metric.jvm.class.loaded` (metric)

### Brief

Number of classes loaded since JVM start.



Prefix: 
Metric: jvm.class.loaded
Instrument: counter
Unit: {class}
Stability: Stable

### Attributes


  
## Group `metric.jvm.class.unloaded` (metric)

### Brief

Number of classes unloaded since JVM start.



Prefix: 
Metric: jvm.class.unloaded
Instrument: counter
Unit: {class}
Stability: Stable

### Attributes


  
## Group `metric.jvm.class.count` (metric)

### Brief

Number of classes currently loaded.



Prefix: 
Metric: jvm.class.count
Instrument: updowncounter
Unit: {class}
Stability: Stable

### Attributes


  
## Group `metric.jvm.cpu.count` (metric)

### Brief

Number of processors available to the Java virtual machine.



Prefix: 
Metric: jvm.cpu.count
Instrument: updowncounter
Unit: {cpu}
Stability: Stable

### Attributes


  
## Group `metric.jvm.cpu.time` (metric)

### Brief

CPU time used by the process as reported by the JVM.



Prefix: 
Metric: jvm.cpu.time
Instrument: counter
Unit: s
Stability: Stable

### Attributes


  
## Group `metric.jvm.cpu.recent_utilization` (metric)

### Brief

Recent CPU utilization for the process as reported by the JVM.

The value range is [0.0,1.0]. This utilization is not defined as being for the specific interval since last measurement (unlike `system.cpu.utilization`). [Reference](https://docs.oracle.com/en/java/javase/17/docs/api/jdk.management/com/sun/management/OperatingSystemMXBean.html#getProcessCpuLoad()).

Prefix: 
Metric: jvm.cpu.recent_utilization
Instrument: gauge
Unit: 1
Stability: Stable

### Attributes


  