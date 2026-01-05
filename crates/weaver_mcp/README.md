# MCP Server for LLM Integration

Weaver includes an MCP (Model Context Protocol) server that exposes the semantic conventions registry to LLMs. This enables natural language queries for finding and understanding conventions while writing instrumentation code.

## Configure Your LLM Client

Follow the steps for your specific LLM client to add the Weaver MCP server. For example, Claude Code:

```bash
# Add globally (available in all projects)
claude mcp add --global --transport stdio weaver \
  /path/to/weaver -- registry mcp

# Or add to current project only
claude mcp add --transport stdio weaver \
  /path/to/weaver -- registry mcp
```

Replace `/path/to/weaver` with the actual path to your weaver binary (e.g., `./target/release/weaver`).

To use a specific registry:

```bash
claude mcp add --global --transport stdio weaver \
  /path/to/weaver -- registry mcp \
  --registry my_project/model
```

## Verify Connection

You should see the weaver tools available. Try asking:

> "Search for HTTP server attributes in semantic conventions"

## Available Tools

The MCP server exposes 7 tools:

| Tool            | Description                                                                     |
| --------------- | ------------------------------------------------------------------------------- |
| `search`        | Search across all registry items (attributes, metrics, spans, events, entities) |
| `get_attribute` | Get detailed information about a specific attribute by key                      |
| `get_metric`    | Get detailed information about a specific metric by name                        |
| `get_span`      | Get detailed information about a specific span by type                          |
| `get_event`     | Get detailed information about a specific event by name                         |
| `get_entity`    | Get detailed information about a specific entity by type                        |
| `live_check`    | Validate telemetry samples against the registry                                 |

### Search Tool

The most commonly used tool. Supports:

- **query**: Search keywords (e.g., "http server", "database connection")
- **type**: Filter by type (`all`, `attribute`, `metric`, `span`, `event`, `entity`)
- **stability**: Filter by stability (`stable`, `experimental`)
- **limit**: Maximum results (default: 20)

### Get Tools

Each get tool retrieves detailed information about a specific item:

- `get_attribute` - Use `key` parameter (e.g., `http.request.method`)
- `get_metric` - Use `name` parameter (e.g., `http.server.request.duration`)
- `get_span` - Use `type` parameter (e.g., `http.server`)
- `get_event` - Use `name` parameter (e.g., `exception`)
- `get_entity` - Use `type` parameter (e.g., `service`)

### Live Check Tool

Validates telemetry samples against the semantic conventions registry. Pass an array of samples (attributes, spans, metrics, logs, or resources) and receive them back with `live_check_result` fields populated containing advice and findings.

Example input:

```json
{
  "samples": [
    { "attribute": { "name": "http.request.method", "value": "GET" } },
    {
      "span": {
        "name": "GET /users",
        "kind": "server",
        "attributes": [
          { "name": "http.request.method", "value": "GET" },
          { "name": "http.response.status_code", "value": 200 }
        ]
      }
    }
  ]
}
```

The tool runs built-in advisors (deprecated, stability, type, enum) to provide feedback on:

- Deprecated attributes/metrics
- Non-stable items (experimental/development)
- Type mismatches (e.g., string vs int)
- Invalid enum values

## Example Prompts

Here are some example prompts:

### Finding Attributes

> "What attributes should I use for HTTP server instrumentation?"

> "Search for database-related attributes"

> "Find all stable attributes for messaging systems"

### Getting Details

> "Get the details for the http.request.method attribute"

> "What is the http.server.request.duration metric?"

### Instrumentation Guidance

> "I'm adding tracing to a gRPC service. What semantic conventions should I follow?"

> "How should I instrument a Redis client according to OpenTelemetry conventions?"
