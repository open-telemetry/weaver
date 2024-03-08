## Group `attributes.jvm.memory` (attribute_group)

### Brief

Describes JVM memory metric attributes.

prefix: jvm.memory

### Attributes


#### Attribute `jvm.memory.type`

The type of memory.


- Requirement Level: Recommended
  
- Type: Enum [heap, non_heap]
- Examples: [
    "heap",
    "non_heap",
]
  
- Stability: Stable
  
  
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
  
  