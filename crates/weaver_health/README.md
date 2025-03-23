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
        Attr["SampleAttribute
        for structure-less data"]
        SpanGroup["SampleSpan
        for structured data"]
        MetricGroup["SampleMetric
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
    TextFile --> Attr
    JSON --> SpanGroup
    Other --> Attr
    
    Attr --> AttrChecks
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

1. **SampleAttribute**
   - Used for structure-less or flat data
   - Contains attribute metadata with optional sample values
   - Supports basic attribute presence and value checks

2. **SampleSpan, SampleMetric, ...**
   - Used for fully structured telemetry data
   - Contains complete metadata e.g. Spans, Metrics, ...
   - Supports comprehensive structural and relationship validation

### 2.3 Data Flow

1. Input data is received by an appropriate ingester
2. The ingester transforms the input into its predetermined output format either attributes only or group types
3. The advice system runs appropriate comparisons on the intermediary format
4. Advice is collected into a list for each entity
5. Results are transformed into various report formats for different downstream uses (coverage reports, statistical analysis, YAML generation, human-readable output)

### 2.4 Advice system

Advisors implement a simple trait and are called during the health-check run. The design allows for simple and high-complexity advisors. For example, a simple snake_case checker or an LLM advisor. The advisors you wish to use are configured at the CLI for the run. Each advisor returns an `Option<Advice>` - None means there's nothing negative to advise on. Here is a JSON output for an attribute level health-check:

```json
[
    {
        "sample_attribute": {
            "name": "aws.s3.bucket"
        },
        "all_advice": []
    },
    {
        "sample_attribute": {
            "name": "aws.s3.bucket.name"
        },
        "all_advice": [
            {
                "key": "attribute_match",
                "value": false,
                "message": "This attribute does not exist in the registry"
            }
        ]
    },
    {
        "sample_attribute": {
            "name": "task.id"
        },
        "all_advice": [
            {
                "key": "attribute_match",
                "value": false,
                "message": "This attribute does not exist in the registry"
            }
        ]
    },
    {
        "sample_attribute": {
            "name": "TaskId"
        },
        "all_advice": [
            {
                "key": "attribute_match",
                "value": false,
                "message": "This attribute does not exist in the registry"
            },
            {
                "key": "correct_case",
                "value": false,
                "message": "This attribute is not in snake case"
            }
        ]
    }
]
```

## 3. Usage Examples

Pipe a list of attribute names or name=value pairs to the `AttributeStdin` ingester.

```sh
cat attributes.txt | weaver registry health --ingester as
```

Or a redirect

```sh
weaver registry health --ingester as < attributes.txt
```

Or a here-doc

```sh
weaver registry health --ingester as << EOF
code.function
thing.blah
EOF
```

Or enter text at the prompt, an empty line will exit

```sh
weaver registry health --ingester as
code.line.number=42
```

Using `emit` for a round-trip test:

```sh
weaver registry health --ingester ao -r ../semantic-conventions/model --output ./outdir &
HEALTH_PID=$!
sleep 3
weaver registry emit -r ../semantic-conventions/model --skip-policies
kill -HUP $HEALTH_PID
wait $HEALTH_PID
```

Vendor example: Health check column names in a Honeycomb dataset

```sh
curl -s -X GET 'https://api.honeycomb.io/1/columns/{dataset}' -H 'X-Honeycomb-Team: {API_KEY}' \
| jq -r '.[].key_name' \
| weaver registry health --ingester as -r ../semantic-conventions/model
```

Receive OTLP requests and output advice as it arrives. Useful for debugging an application to check for telemetry problems as you step through your code. (ctrl-c to exit, or wait for the timeout)

```sh
weaver registry health --ingester ao -r ../semantic-conventions/model --inactivity-timeout 120
```

CI/CD - create a JSON report

```sh
weaver registry health --ingester ao -r ../semantic-conventions/model --format json --output ./outdir &
HEALTH_PID=$!
sleep 3
# Run the code under test here.
kill -HUP $HEALTH_PID
wait $HEALTH_PID
# Check the exit code and/or parse the JSON in outdir
```
