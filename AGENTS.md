# PROJECT KNOWLEDGE BASE

**Generated:** 2025-01-06T10:37:00Z
**Commit:** UNKNOWN  
**Branch:** UNKNOWN

## OVERVIEW
OpenTelemetry Weaver - Rust CLI tool for managing semantic convention registries and telemetry schema workflows with observability-by-design principles.

## STRUCTURE
```
./
├── crates/              # Rust workspace members
│   ├── weaver_semconv/     # Core semantic convention processing
│   ├── weaver_forge/       # Code generation engine  
│   ├── weaver_resolved_schema/ # Schema resolution
│   ├── weaver_live_check/   # Real-time validation
│   └── [other crates]       # Specialized utilities
├── src/                  # Main CLI application
│   ├── registry/           # Registry command implementations
│   ├── serve/              # API server functionality
│   └── [cli modules]      # Command orchestration
├── docs/                  # Documentation
├── tests/                 # Integration tests
└── ui/                   # Web UI components
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| CLI entry point | `src/main.rs` | Main command orchestration |
| Registry commands | `src/registry/` | All registry CLI logic |
| Code generation | `crates/weaver_forge/` | Template engine + output formats |
| Semantic conventions | `crates/weaver_semconv/` | Core semconv processing |
| Schema resolution | `crates/weaver_resolved_schema/` | V1/V2 schema handling |
| Live validation | `crates/weaver_live_check/` | Real-time OTLP checking |
| API server | `src/serve/` | Experimental HTTP API |

## CONVENTIONS
- **Rust workspace** with `crates/*` members
- **Strict linting**: `cargo clippy` denies by default, `missing_docs = deny`
- **Error handling**: `miette` for rich diagnostics
- **CLI patterns**: `clap` derive macros with subcommands
- **Async**: `tokio` runtime throughout
- **Logging**: `env_logger` with configurable levels
- **Testing**: Unit tests in `tests/`, integration tests separate

## ANTI-PATTERNS (THIS PROJECT)
- `print_stdout/print_stderr` macros are denied
- `unwrap_used` is denied - explicit error handling required
- `dbg_macro` is denied in production code
- Multiple crate versions allowed (but warned)
- TODO/FIXME comments should be addressed - 100+ instances found

## UNIQUE STYLES
- **Versioned schemas**: V1 and V2 parallel support for backward compatibility
- **Embedded diagnostics**: Rich error reporting with template system
- **Registry-first**: Schema definition drives code generation, not vice-versa
- **OTLP integration**: Direct gRPC/HTTP telemetry stream processing
- **Template-driven**: Jinja-like templating for code generation

## COMMANDS
```bash
# Development
cargo build --release
cargo test --workspace
cargo clippy --workspace

# Registry operations  
weaver registry check
weaver registry resolve
weaver registry generate

# Validation
weaver registry live-check
weaver registry emit
```

## NOTES
- **Experimental features**: Flag required for new validation rules
- **Cross-platform**: Windows/Linux/macOS CI testing
- **Monorepo structure**: Each crate has distinct responsibility
- **Documentation-driven**: README-first development approach
- **OpenTelemetry standards**: Direct alignment with OTel spec
- **UI Development**: Both Svelte (`ui/`) and React (`ui-react/`) apps maintain parallel development
  - Svelte uses Vite + Svelte + TypeScript
  - React uses Vite + React + TypeScript with TanStack Router
  - Both apps share same Tailwind + DaisyUI styling and API endpoints
  - Development servers run on different ports (ui: 4173, ui-react: 5173)
  - Theme toggle already implemented in React via useState + useEffect hooks
- **Build process**: `build.rs` automatically builds the React UI (`ui-react/`) during `cargo build` via npm
  - This ensures UI assets are embedded in the binary for serving
  - Requires Node.js and npm to be installed on build machine