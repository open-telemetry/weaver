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

