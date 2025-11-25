# Setup Weaver Action

Install [OpenTelemetry Weaver](https://github.com/open-telemetry/weaver) CLI in your GitHub Actions workflows.

## Quick Start

```yaml
steps:
  - uses: actions/checkout@v4

  - name: Setup Weaver
    uses: open-telemetry/weaver/.github/actions/setup-weaver@main
    # with:
    #   version: '0.18.0'  # Optional: pin to specific version

  - name: Validate semantic conventions
    run: weaver registry check -r ./model --diagnostic-format gh_workflow_command
```

For complete workflow examples, see [opentelemetry-weaver-examples](https://github.com/open-telemetry/opentelemetry-weaver-examples).

## Inputs

| Input | Description | Required | Default |
|-------|-------------|----------|---------|
| `version` | Weaver version to install (e.g., `0.18.0`). Omit for latest release. | No | `''` (latest) |
| `cache` | Enable caching of Weaver binary for faster subsequent runs | No | `'true'` |

## Outputs

| Output | Description |
|--------|-------------|
| `version` | Installed Weaver version (e.g., `v0.18.0`) |

## Caching

By default, this action caches the Weaver binary to speed up subsequent runs.

- **Cache key**: `setup-weaver-{os}-{arch}-{version}`
- **First run**: ~30-60 seconds (download + install)
- **Cached run**: ~2-5 seconds (restore from cache)

To disable caching:
```yaml
- uses: open-telemetry/weaver/.github/actions/setup-weaver@main
  with:
    cache: 'false'
```

## Platform Support

| Platform | Architecture | Status |
|----------|-------------|---------|
| Ubuntu (Linux) | x86_64 | ✅ Supported |
| macOS | x86_64 (Intel) | ✅ Supported |
| macOS | aarch64 (Apple Silicon) | ✅ Supported |
| Windows | x86_64 | ✅ Supported |

The action automatically detects your platform and architecture.

## Troubleshooting

**Command not found**: Ensure the setup step completed successfully and you're running commands after the setup step.

**Cache issues**: Temporarily disable caching with `cache: 'false'` or clear cache in repository settings (Actions → Caches).

**Version issues**: Pin to a known working version with `version: '0.18.0'`.

For more information:
- [opentelemetry-weaver-examples](https://github.com/open-telemetry/opentelemetry-weaver-examples) - Complete workflow examples
- [Weaver documentation](https://github.com/open-telemetry/weaver/tree/main/docs) - CLI commands and usage
- [Weaver releases](https://github.com/open-telemetry/weaver/releases) - Version history and changelogs
