# Weaver Health

## 1. Overview

The weaver-health system provides a decoupled, flexible, extensible framework for health checking sample telemetry. It transforms various input formats into standardized intermediary representations that can be assessed by advisors against expected patterns or values. The output results use the standard weaver templated process for full flexibility.

```mermaid
flowchart TD
    subgraph Ingesters
        OTLP[OTLP Ingester]
        TextFile[Text File Ingester]
        JSON[JSON Ingester]
        Other[Other Ingesters...]
    end
    
    subgraph "Intermediary Format"
        VecAttr["Vec&lt;SampleAttribute>
        for structure-less data"]
        SpanGroup["Vec&lt;SampleSpan>
        for structured data"]
        MetricGroup["Vec&lt;SampleMetric>
        for structured data"]
    end
    
    subgraph "Advice System"
        AttrChecks[Attribute advisors
        - Existence
        - Type compatibility
        - Value validation]
        StructChecks[Structural advisors
        - Group validity
        - Relationships
        - Constraints]
        Results[Advice]
    end
    
    subgraph "Report Formats"
        CoverageRep[Coverage Report]
        StatsRep[Statistical Report]
        YamlGen[SemConv YAML Generation]
        HumanRep[Human-Readable Output]
    end
    
    OTLP --> SpanGroup
    OTLP --> MetricGroup
    TextFile --> VecAttr
    JSON --> SpanGroup
    Other --> VecAttr
    
    VecAttr --> AttrChecks
    SpanGroup --> AttrChecks
    SpanGroup --> StructChecks
    MetricGroup --> AttrChecks
    MetricGroup --> StructChecks
    
    AttrChecks --> Results
    StructChecks --> Results
    
    Results --> CoverageRep
    Results --> StatsRep
    Results --> YamlGen
    Results --> HumanRep
    
    CoverageRep -.-> CI[CI/CD Integration]
    StatsRep -.-> Analytics[Analytics Systems]
    YamlGen -.-> Registry[Registry Enhancement]
    HumanRep -.-> UI[Dashboards/UI]
```

## 2. Architecture

### 2.1 Core Components

The weaver-health system comprises four main components:

1. **Ingesters**: Transform specific input formats into intermediary formats
2. **Intermediary Formats**: Standardized representations of telemetry data
3. **Advice System**: Compares intermediary data with expected patterns
4. **Report Formats**: Structured outputs for various downstream uses

### 2.2 Intermediary Formats

Intermediary formats are used for attributes and groups:

1. **Vec\<SampleAttribute\>**
   - Used for structure-less or flat data
   - Contains attribute metadata with optional sample values
   - Supports basic attribute presence and value checks

2. **Vec\<SampleSpan\>, Vec\<SampleMetric\>, ...**
   - Used for fully structured telemetry data
   - Contains complete metadata e.g. Spans, Metrics, ...
   - Supports comprehensive structural and relationship validation

### 2.3 Data Flow

1. Input data is received by an appropriate ingester
2. The ingester transforms the input into its predetermined output format either attributes only or group types
3. The advice system runs appropriate comparisons on the intermediary format
4. Advice is collected into a list for each entity
5. Results are transformed into various report formats for different downstream uses (coverage reports, statistical analysis, YAML generation, human-readable output)

