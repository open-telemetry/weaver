# Group `metric.jvm.gc.duration` (metric)

## Brief

Duration of JVM garbage collection actions.

prefix: jvm.gc

## Attributes


### Attribute `jvm.gc.name`

Name of the garbage collector.


Garbage collector name is generally obtained via [GarbageCollectionNotificationInfo#getGcName()](https://docs.oracle.com/en/java/javase/11/docs/api/jdk.management/com/sun/management/GarbageCollectionNotificationInfo.html#getGcName()).

- Requirement Level: Recommended

- Type: string
- Examples: [
    "G1 Young Generation",
    "G1 Old Generation",
]

- Stability: Stable


### Attribute `jvm.gc.action`

Name of the garbage collector action.


Garbage collector action is generally obtained via [GarbageCollectionNotificationInfo#getGcAction()](https://docs.oracle.com/en/java/javase/11/docs/api/jdk.management/com/sun/management/GarbageCollectionNotificationInfo.html#getGcAction()).

- Requirement Level: Recommended

- Type: string
- Examples: [
    "end of minor GC",
    "end of major GC",
]

- Stability: Stable



## Provenance

Source: data/jvm-metrics.yaml

