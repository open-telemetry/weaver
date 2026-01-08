# Registry Validation

<kbd>weaver registry check</kbd> ([Usage](usage.md#registry-check))

The validation process for a semantic convention registry involves several steps:
- Loading the semantic convention specifications from a local directory or a git repository.
- Parsing the loaded semantic convention specifications.
- Resolving references and extends clauses within the specifications.
- Checking compliance with specified Rego policies, if provided.


## Custom rules (Rego)

Specific validation rules can be expressed using the Rego policy language (https://www.openpolicyagent.org/docs/policy-language).

Please see [Weaver Checker](https://github.com/open-telemetry/weaver/blob/main/crates/weaver_checker/README.md) for details.

## Finding Structure

When a custom Rego policy detects an issue, it returns a finding object. The structure of this object is used by Weaver to report warnings and violations.

### Fields

A finding object contains the following fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | A short, machine-readable identifier that categorizes the finding (e.g., `"is_deprecated"`, `"invalid_format"`) |
| `context` | object | Yes | A JSON object containing all dynamic information about the finding. Values here can be used with custom templates and filters to generate reports |
| `message` | string | Yes | A human-readable description of the finding that explains what was detected and how to fix it |
| `level` | string | Yes | The severity level: `"information"`, `"improvement"`, or `"violation"` |
| `signal_type` | string | No | The type of telemetry signal this finding applies to: `"span"`, `"metric"`, `"log"`, `"entity"`, or `"profile"` |
| `signal_name` | string | No | The specific signal name this finding applies to (e.g., `"http.server.request.duration"`) |

### Finding Levels

The `level` field indicates the severity and expected action:

- **`information`**: Useful context without requiring action. Used for informational messages that help users understand their telemetry schema.
- **`improvement`**: A suggested change that would improve the quality or maintainability of the schema. Not required but recommended.
- **`violation`**: Something that breaks compliance rules and must be fixed before the schema can be published.

### Creating custom findings in Rego

To write custom Rego policies, refer to the [Rego Policy Language documentation](https://www.openpolicyagent.org/docs/policy-language) for syntax and language features. For more details on the Weaver Policy Engine, see the [Weaver Checker documentation](/crates/weaver_checker/README.md).

Here's how to create custom findings in your Rego policy:

```rego
package after_resolution

# Example: Validate attribute names contain only alphanumeric chars, dots, or underscores
deny contains finding if {
    # These lines assume the input is following schema v1
    group := input.groups[_]
    attr := group.attributes[_]
    
    # Check if attribute name contains invalid characters
    not regex.match("^[a-zA-Z0-9._]+$", attr.name)
    
    finding := {
        "id": "invalid_attribute_name",
        "context": {
            "attribute_name": attr.name,
            "group_id": group.id
        },
        "message": sprintf("Attribute '%s' in group '%s' contains invalid characters. Only alphanumeric characters, dots, and underscores are allowed.", 
            [attr.name, group.id]),
        "level": "violation"
    }
}

# Example: Suggest an improvement
deny contains finding if {
    metric := input.metrics[_]
    not metric.unit
    
    finding := {
        "id": "missing_metric_unit",
        "context": {
            "metric_name": metric.name
        },
        "message": sprintf("Metric '%s' should define a unit for better observability", 
            [metric.name]),
        "level": "improvement",
        "signal_type": "metric",
        "signal_name": metric.name
    }
}
```

### Exporting Findings

The `weaver registry check` command supports multiple output formats via the `--diagnostic-format` flag:

#### JSON Format

Use `--diagnostic-format json` to export findings as a JSON array. This is useful for programmatic processing or integration with other tools:

```bash
weaver registry check \
  --registry ./my-registry \
  --policy ./my-policies \
  --diagnostic-format json \
  2> findings.json
```

The JSON output is an array of diagnostic messages, each containing a finding:

```json
[
  {
    "id": "invalid_attribute_name",
    "context": {
      "attribute_name": "http-method",
      "group_id": "registry.http"
    },
    "message": "Attribute 'http-method' in group 'registry.http' contains invalid characters. Only alphanumeric characters, dots, and underscores are allowed.",
    "level": "violation"
  },
  {
    "id": "missing_metric_unit",
    "context": {
      "metric_name": "http.server.request.duration"
    },
    "message": "Metric 'http.server.request.duration' should define a unit for better observability",
    "level": "improvement",
    "signal_type": "metric",
    "signal_name": "http.server.request.duration"
  }
]
```

#### Other Formats

- **`ansi`** (default): Human-readable output with color and formatting for terminal display
- **`gh_workflow_command`**: GitHub Actions workflow commands format for CI/CD integration

### Backward Compatibility

For backward compatibility, Weaver still accepts legacy finding formats:

- **Legacy semconv_attribute format** (deprecated):
  ```json
  {
    "type": "semconv_attribute",
    "id": "...",
    "category": "...",
    "group": "...",
    "attr": "..."
  }
  ```

- **Legacy advice format** (deprecated):
  ```json
  {
    "type": "advice",
    "advice_type": "...",
    "advice_level": "...",
    "advice_context": {...},
    "message": "..."
  }
  ```

These formats are automatically converted internally. **For new policies, use the finding structure shown above.**
