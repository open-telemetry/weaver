# MCP Server for LLM Integration

Weaver includes an MCP (Model Context Protocol) server that exposes the semantic conventions registry to LLMs. This enables natural language queries for finding and understanding conventions while writing instrumentation code.

Supported clients:
- [Claude Code / Claude Desktop](#configure-claude-code)
- [GitHub Copilot](#configure-github-copilot)

## Quick Start

### 1. Build Weaver

```bash
cargo build --release
```

### 2. Configure Your LLM Client

#### Configure Claude Code

Add the MCP server using the Claude CLI:

```bash
# Add globally (available in all projects)
claude mcp add --global --transport stdio weaver \
  /path/to/weaver registry mcp

# Or add to current project only
claude mcp add --transport stdio weaver \
  /path/to/weaver registry mcp
```

Replace `/path/to/weaver` with the actual path to your weaver binary (e.g., `./target/release/weaver`).

To use a specific registry:

```bash
claude mcp add --global --transport stdio weaver \
  /path/to/weaver registry mcp \
  --registry https://github.com/open-telemetry/semantic-conventions.git
```

#### Alternative: Manual Configuration

You can also manually edit the Claude Code configuration file:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Linux**: `~/.config/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "weaver": {
      "command": "/path/to/weaver",
      "args": [
        "registry",
        "mcp",
        "--registry",
        "https://github.com/open-telemetry/semantic-conventions.git"
      ]
    }
  }
}
```

#### Configure GitHub Copilot

Add the MCP server to your VS Code settings (`settings.json`):

```json
{
  "github.copilot.chat.mcp.servers": {
    "weaver": {
      "command": "/path/to/weaver",
      "args": [
        "registry",
        "mcp",
        "--registry",
        "https://github.com/open-telemetry/semantic-conventions.git"
      ]
    }
  }
}
```

Or add to workspace settings (`.vscode/settings.json`) for project-specific configuration.

Replace `/path/to/weaver` with the actual path to your weaver binary.

### 3. Restart Your Editor

After configuration, restart your editor/client to load the MCP server.

### 4. Verify Connection

You should see the weaver tools available. Try asking:

> "Search for HTTP server attributes in semantic conventions"

## Command Usage

```bash
# With OpenTelemetry semantic conventions (default)
weaver registry mcp --registry https://github.com/open-telemetry/semantic-conventions.git

# With a local registry
weaver registry mcp --registry /path/to/local/registry

# Specify registry path within the repo (default: "model")
weaver registry mcp --registry https://github.com/my-org/my-conventions.git --registry-path model
```

Custom registries must follow the [Weaver registry format](./registry.md).

## Available Tools

The MCP server exposes 7 tools:

| Tool | Description |
|------|-------------|
| `search` | Search across all registry items (attributes, metrics, spans, events, entities) |
| `get_attribute` | Get detailed information about a specific attribute by key |
| `get_metric` | Get detailed information about a specific metric by name |
| `get_span` | Get detailed information about a specific span by type |
| `get_event` | Get detailed information about a specific event by name |
| `get_entity` | Get detailed information about a specific entity by type |
| `live_check` | Validate telemetry samples against the registry |

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
    {"attribute": {"name": "http.request.method", "value": "GET"}},
    {"span": {"name": "GET /users", "kind": "server", "attributes": [
      {"name": "http.request.method", "value": "GET"},
      {"name": "http.response.status_code", "value": 200}
    ]}}
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

## Troubleshooting

### Server doesn't start

1. Check the path to the weaver binary is correct
2. Verify the registry URL is accessible
3. Check logs for error messages:
   - **Claude Code**: Check the MCP server logs in the output panel
   - **GitHub Copilot**: Check the VS Code Output panel → "GitHub Copilot Chat"

### No tools available

1. Ensure the configuration JSON is valid
2. Restart your editor after configuration changes
3. Check that the MCP server process is running

### Slow startup

The first run may be slow as it clones the semantic conventions repository. Subsequent runs use a cached version.

### Using a local registry

For faster startup during development, clone the registry locally:

```bash
git clone https://github.com/open-telemetry/semantic-conventions.git /path/to/semconv

# Then use local path
weaver registry mcp --registry /path/to/semconv
```

## Architecture

The MCP server:

1. Loads the semantic conventions registry into memory at startup
2. Communicates via JSON-RPC 2.0 over stdio
3. Provides direct memory access to registry data (no HTTP overhead)
4. Runs as a single process managed by the LLM client

```
LLM Client <-- JSON-RPC (stdio) --> weaver registry mcp <-- memory --> Registry
```

