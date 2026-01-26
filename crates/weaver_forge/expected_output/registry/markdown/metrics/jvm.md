# Metrics: `jvm`

This document describes the `jvm` metrics.

## `jvm.class.count`

Number of classes currently loaded.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `{class}` |
| Stability | Stable |

## `jvm.class.loaded`

Number of classes loaded since JVM start.

| Property | Value |
|----------|-------|
| Instrument | counter |
| Unit | `{class}` |
| Stability | Stable |

## `jvm.class.unloaded`

Number of classes unloaded since JVM start.

| Property | Value |
|----------|-------|
| Instrument | counter |
| Unit | `{class}` |
| Stability | Stable |

## `jvm.cpu.count`

Number of processors available to the Java virtual machine.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `{cpu}` |
| Stability | Stable |

## `jvm.cpu.recent_utilization`

Recent CPU utilization for the process as reported by the JVM.

The value range is [0.0,1.0]. This utilization is not defined as being for the specific interval since last measurement (unlike `system.cpu.utilization`). [Reference](https://docs.oracle.com/en/java/javase/17/docs/api/jdk.management/com/sun/management/OperatingSystemMXBean.html#getProcessCpuLoad()).

| Property | Value |
|----------|-------|
| Instrument | gauge |
| Unit | `1` |
| Stability | Stable |

## `jvm.cpu.time`

CPU time used by the process as reported by the JVM.

| Property | Value |
|----------|-------|
| Instrument | counter |
| Unit | `s` |
| Stability | Stable |

## `jvm.gc.duration`

Duration of JVM garbage collection actions.

| Property | Value |
|----------|-------|
| Instrument | histogram |
| Unit | `s` |
| Stability | Stable |

### Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `jvm.gc.name` | `string` | No | Name of the garbage collector. |
| `jvm.gc.action` | `string` | No | Name of the garbage collector action. |

## `jvm.memory.committed`

Measure of memory committed.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `By` |
| Stability | Stable |

### Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `jvm.memory.type` | Enum | No | The type of memory. |
| `jvm.memory.pool.name` | `string` | No | Name of the memory pool. |

## `jvm.memory.deprecated`

Measure something deprecated.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `By` |
| Stability | Stable |
| Deprecated | Use `metric.jvm.memory.used` instead. |

## `jvm.memory.limit`

Measure of max obtainable memory.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `By` |
| Stability | Stable |

### Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `jvm.memory.type` | Enum | No | The type of memory. |
| `jvm.memory.pool.name` | `string` | No | Name of the memory pool. |

## `jvm.memory.used`

Measure of memory used.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `By` |
| Stability | Stable |

### Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `jvm.memory.type` | Enum | No | The type of memory. |
| `jvm.memory.pool.name` | `string` | No | Name of the memory pool. |
| `jvm.memory.deprecated.attribute` | `boolean` | No | Something deprecated. |
| `jvm.memory.experimental.attribute` | `boolean` | No | Something experimental. |
| `jvm.memory.stable.attribute` | `boolean` | No | Something stable. |

## `jvm.memory.used_after_last_gc`

Measure of memory used, as measured after the most recent garbage collection event on this pool.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `By` |
| Stability | Stable |

### Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `jvm.memory.type` | Enum | No | The type of memory. |
| `jvm.memory.pool.name` | `string` | No | Name of the memory pool. |

## `jvm.thread.count`

Number of executing platform threads.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `{thread}` |
| Stability | Stable |

### Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `jvm.thread.daemon` | `boolean` | No | Whether the thread is daemon or not. |
| `jvm.thread.state` | Enum | No | State of the thread. |

