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
  
  