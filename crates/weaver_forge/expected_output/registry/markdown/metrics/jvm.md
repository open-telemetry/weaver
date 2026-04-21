# Metrics: `jvm`

This document describes the `jvm` metrics.

## `jvm.class.count`
This metric is recommended.

Number of classes currently loaded.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `{class}` |
| Stability | Stable |

## `jvm.class.loaded`
This metric is recommended.

Number of classes loaded since JVM start.

| Property | Value |
|----------|-------|
| Instrument | counter |
| Unit | `{class}` |
| Stability | Stable |

## `jvm.class.unloaded`
This metric is recommended.

Number of classes unloaded since JVM start.

| Property | Value |
|----------|-------|
| Instrument | counter |
| Unit | `{class}` |
| Stability | Stable |

## `jvm.cpu.count`
This metric is recommended.

Number of processors available to the Java virtual machine.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `{cpu}` |
| Stability | Stable |

## `jvm.cpu.recent_utilization`
This metric is recommended.

Recent CPU utilization for the process as reported by the JVM.

The value range is [0.0,1.0]. This utilization is not defined as being for the specific interval since last measurement (unlike `system.cpu.utilization`). [Reference](https://docs.oracle.com/en/java/javase/17/docs/api/jdk.management/com/sun/management/OperatingSystemMXBean.html#getProcessCpuLoad()).

| Property | Value |
|----------|-------|
| Instrument | gauge |
| Unit | `1` |
| Stability | Stable |

## `jvm.cpu.time`
This metric is recommended.

CPU time used by the process as reported by the JVM.

| Property | Value |
|----------|-------|
| Instrument | counter |
| Unit | `s` |
| Stability | Stable |

## `jvm.gc.duration`
This metric is recommended.

Duration of JVM garbage collection actions.

| Property | Value |
|----------|-------|
| Instrument | histogram |
| Unit | `s` |
| Stability | Stable |

### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `jvm.gc.action` | `string` | Recommended | Name of the garbage collector action.
 |
| `jvm.gc.name` | `string` | Recommended | Name of the garbage collector.
 |

## `jvm.memory.committed`
This metric is recommended.

Measure of memory committed.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `By` |
| Stability | Stable |

### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `jvm.memory.pool.name` | `string` | Recommended | Name of the memory pool.
 |
| `jvm.memory.type` | Enum | Recommended | The type of memory.
 |

## `jvm.memory.deprecated`
This metric is .

Measure something deprecated.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `By` |
| Stability | Stable |
| Deprecated | Use `metric.jvm.memory.used` instead. |

## `jvm.memory.limit`
This metric is recommended.

Measure of max obtainable memory.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `By` |
| Stability | Stable |

### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `jvm.memory.pool.name` | `string` | Recommended | Name of the memory pool.
 |
| `jvm.memory.type` | Enum | Recommended | The type of memory.
 |

## `jvm.memory.used`
This metric is recommended.

Measure of memory used.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `By` |
| Stability | Stable |

### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `jvm.memory.deprecated.attribute` | `boolean` | Recommended | Something deprecated.
 |
| `jvm.memory.pool.name` | `string` | Recommended | Name of the memory pool.
 |
| `jvm.memory.stable.attribute` | `boolean` | Recommended | Something stable.
 |
| `jvm.memory.type` | Enum | Recommended | The type of memory.
 |
| `jvm.memory.experimental.attribute` | `boolean` | Opt-In | Something experimental.
 |

## `jvm.memory.used_after_last_gc`
This metric is recommended.

Measure of memory used, as measured after the most recent garbage collection event on this pool.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `By` |
| Stability | Stable |

### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `jvm.memory.pool.name` | `string` | Recommended | Name of the memory pool.
 |
| `jvm.memory.type` | Enum | Recommended | The type of memory.
 |

## `jvm.thread.count`
This metric is recommended.

Number of executing platform threads.

| Property | Value |
|----------|-------|
| Instrument | updowncounter |
| Unit | `{thread}` |
| Stability | Stable |

### Attributes

| Attribute | Type | Requirement Level | Description |
|-----------|------|-------------------|-------------|
| `jvm.thread.daemon` | `boolean` | Recommended | Whether the thread is daemon or not.
 |
| `jvm.thread.state` | Enum | Recommended | State of the thread.
 |

