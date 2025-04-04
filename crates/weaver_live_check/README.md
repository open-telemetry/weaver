# Weaver Live Check

Live check is a developer tool for assessing sample telemetry and providing advice for improvement.

A Semantic Convention `Registry` is loaded for comparison with samples. `Ingesters` transform various input formats and sources into intermediary representations to be assessed by `Advisors`. The `Advice` produced is transformed via jinja templates to the required output format for downstream consumption.

```mermaid
flowchart LR
    subgraph Inputs
        file["File"]
        stdin["stdin"]
        otlp["OTLP"]
    end

    subgraph Core["Processing"]
        registry["Registry"]
        ingesters["Ingesters"]

        subgraph advisors["Advisors"]
            builtin["Builtin"]

            subgraph external["External"]
                otel["Otel"]
                custom["Custom"]
            end
        end
    end

    subgraph Outputs
        advice["Advice"]
        templates["Jinja Templates"]
        output["Output Format"]
    end

    file --> ingesters
    stdin --> ingesters
    otlp --> ingesters

    registry -- "Loaded for comparison" --> advisors
    ingesters -- "Intermediary representations" --> advisors

    builtin --> advice
    external --> advice

    advice -- "Transformed via" --> templates
    templates --> output
```

## Ingesters

Sample data can have various levels of detail; from a simple list of attribute names, to a full OTLP signal structure. This data can come from different sources: files, stdin, OTLP. Therefore you need to choose the appropriate `Ingester` for your job:

- `attribute_file | AF | af` => for attribute name or name=value pair text files
- `attribute_stdin | AS | as` => for attribute name or name=value pairs on stdin
- `attribute_json_file | AJF | ajf` => for an array of attribute samples in a json file
- `attribute_json_stdin | AJS | ajs` => for an array of attribute samples in json on stdin
- `attribute_otlp | AO | ao` => for attributes extracted from OTLP signals

Some `Ingesters`, like `attribute_stdin` and `attribute_otlp`, can stream the input data so you receive output at the command line as it comes in. This is really useful in live debugging sessions allowing you to breakpoint, step through your code and see live assessment as the data is received in Weaver.

## Advisors

Sample entities are assessed by the set of `Advisors` to produce a list of `Advice` for each one. Built-ins check for fundamental compliance with the `Registry` supplied, for example `missing_attribute` and `type_mismatch`.

Beyond the fundamentals, external `Advisors` can be defined in Rego policies. The OpenTelemetry Semantic Conventions rules are included out-of-the-box by default. They provide `Advice` on name-spacing and formatting aligned with the standard. These default policies can be overridden at the command line with your own.

### Advice

As mentioned, a list of `Advice` is returned in the report for each sample entity. The snippet below shows `Advice` from two `Advisors`. A builtin is providing `missing_attribute` and a default Otel Rego policy is providing `extends_namespace`. The fields of `Advice` are intended to be used like so:

- `advisory`: _string_ - one of `violation`, `improvement` or `information` with that order of precedence. Weaver will return with a non-zero exit-code if there is any `violation` in the report.
- `key`: _string_ - a simple machine readable string key to represent the advice type
- `message`: _string_ - a verbose string describing the advice
- `value`: _any_ - a pertinent entity associated with the advice

```json
{
  "all_advice": [
    {
      "advisory": "violation",
      "key": "missing_attribute",
      "message": "Does not exist in the registry",
      "value": "aws.s3.extension.name"
    },
    {
      "advisory": "information",
      "key": "extends_namespace",
      "message": "Extends existing namespace",
      "value": "aws.s3"
    }
  ],
  "highest_advisory": "violation",
  "sample_attribute": {
    "name": "aws.s3.extension.name",
    "type": "string",
    "value": "foo"
  }
}
```

### Custom advisors

Use the `--advice-policies` command line option to provide a path to a directory containing Rego policies with the `advice` package name. Here's a very simple example that rejects any attribute name containing the string "test":

```rego
package advice

import rego.v1

# checks attribute name contains the word "test"
deny contains make_advice(key, advisory, value, message) if {
	contains(input.name, "test")
	key := "contains_test"
	advisory := "violation"
	value := input.name
	message := "Name must not contain 'test'"
}

make_advice(key, advisory, value, message) := {
	"type": "advice",
	"key": key,
	"advisory": advisory,
	"value": value,
	"message": message,
}
```

`input` contains the sample entity for assessment. `data` contains a structure derived from the supplied `Registry`. A jq preprocessor takes the `Registry` (and maps for attributes and templates) to produce the `data` for the policy. If the jq is simply `.` this will passthrough as-is. Preprocessing is used to improve Rego performance and to simplify policy definitions. With this model `data` is processed once whereas the Rego policy runs for every sample entity as it arrives in the stream. Preprocessing for the default otel policies lead to a ~10x speed-up.

To override the default Otel jq preprocessor provide a path to the jq file through the `--advice-preprocessor` option.

## Output

The output follows existing Weaver paradigms providing overridable jinja template based processing.

Out-of-the-box the output is streamed (when available) to templates providing `ansi` (default) or `json` output via the `--format` option. To override streaming and only produce a report when the input is closed, use `--stream false`. Streaming is automatically disabled if your `--output` is a path to a directory; by default, output is printed to stdout.

To provide your own custom templates use the `--templates` option.

As mentioned, the exit-code is set non-zero if any `violation` advice is provided in the output. This can be used in tests and/or CI to fail builds for example.

### Statistics

A statistics entity is produced when the input is closed like this snippet:

```json
{
  "advice_key_counts": {
    "extends_namespace": 2,
    "illegal_namespace": 1,
    "invalid_format": 1,
    "missing_attribute": 4,
    "missing_namespace": 1,
    "stability": 1,
    "type_mismatch": 1
  },
  "advisory_counts": {
    "improvement": 2,
    "information": 2,
    "violation": 7
  },
  "highest_advisory_counts": {
    "improvement": 1,
    "violation": 5
  },
  "no_advice_count": 1,
  "total_advisories": 11,
  "total_attributes": 7
}
```

This could be parsed for a more sophisticated way to determine pass/fail in CI for example.

## Usage examples

Pipe a list of attribute names or name=value pairs

```sh
cat attributes.txt | weaver registry live-check --ingester as
```

Or a redirect

```sh
weaver registry live-check --ingester as < attributes.txt
```

Or a here-doc

```sh
weaver registry live-check --ingester as << EOF
code.function
thing.blah
EOF
```

Or enter text at the prompt, an empty line will exit

```sh
weaver registry live-check --ingester as
code.line.number=42
```

Using `emit` for a round-trip test:

```sh
weaver registry live-check --ingester ao -r ../semantic-conventions/model --output ./outdir &
LIVE_CHECK_PID=$!
sleep 3
weaver registry emit -r ../semantic-conventions/model --skip-policies
kill -HUP $LIVE_CHECK_PID
wait $LIVE_CHECK_PID
```

Vendor example: Live check column names in a Honeycomb dataset

```sh
curl -s -X GET 'https://api.honeycomb.io/1/columns/{dataset}' -H 'X-Honeycomb-Team: {API_KEY}' \
| jq -r '.[].key_name' \
| weaver registry live-check --ingester as -r ../semantic-conventions/model
```

Receive OTLP requests and output advice as it arrives. Useful for debugging an application to check for telemetry problems as you step through your code. (ctrl-c to exit, or wait for the timeout)

```sh
weaver registry live-check --ingester ao -r ../semantic-conventions/model --inactivity-timeout 120
```

CI/CD - create a JSON report

```sh
weaver registry live-check --ingester ao -r ../semantic-conventions/model --format json --output ./outdir &
LIVE_CHECK_PID=$!
sleep 3
# Run the code under test here.
kill -HUP $LIVE_CHECK_PID
wait $LIVE_CHECK_PID
# Check the exit code and/or parse the JSON in outdir
```
