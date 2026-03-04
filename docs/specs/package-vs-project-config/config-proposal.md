# Proposal: Package Config vs Project Config

## Background

Weaver already has configuration files that sit alongside the artifacts they describe. A `registry_manifest.yaml` lives at the root of a semantic convention registry and declares its name, version, and dependencies. A `weaver.yaml` lives at the root of a template package and defines how those templates are processed — syntax delimiters, comment formats, text maps, default params, and application modes.

This pattern — config adjacent to artifacts — works well because it keeps configuration close to the thing it configures. The config belongs to the package: it's authored, versioned, and distributed together with the artifacts.

## Problem

Today `weaver.yaml` is the only configuration mechanism, and it's being asked to serve two roles. Beyond configuring the template package it sits in, it also supports directory-walking and a `$HOME/.weaver/weaver.yaml` fallback — features designed to let users customize behavior from outside the package. This conflation creates confusion:

- A `weaver.yaml` in a parent directory or `$HOME` silently merges into every template package loaded below it. It's unclear where a setting originates.
- New features like live-check finding modification don't belong in template package config, but there's nowhere else to put project-level concerns.

## Proposal

### 1. Rename `weaver.yaml` to `template_config.yaml`

The name `weaver.yaml` is too generic — it sounds like it configures Weaver itself, when it actually configures a template package. Renaming to `template_config.yaml` makes its purpose explicit: it is the configuration for the templates it sits alongside.

`weaver.yaml` will continue to be loaded as a fallback during a deprecation period, with a warning directing package authors to rename to `template_config.yaml`.

### 2. Load template config from the package directory only

`template_config.yaml` is read from the given template directory and nowhere else. No directory-walking. No `$HOME/.weaver/` fallback. This is already the behavior for remote template packages and the update-markdown command — we are making local packages consistent with remote ones.

A template package should be self-contained and predictable. The same package produces the same output regardless of what `weaver.yaml` files happen to exist in parent directories or the user's home directory.

### 3. Introduce `.weaver.toml` as project config

A new `.weaver.toml` file belongs to the **project** — the codebase that uses Weaver. It configures everything that does not belong to a template package: settings that are CLI-configured today, plus new project-level settings that are cumbersome to provide on the command line.

**What it configures:**

- Live-check finding overrides and filters
- Live-check OTLP and emit settings
- Default registry path, template path, policies path
- Any other setting that is currently a CLI flag

**Where it lives:** In the project repository. Discovered by walking up from the current working directory. A `--config` flag provides an explicit override.

**Example:**

```toml
# === Phase 1: Live-check finding modification ===

[[live_check.finding_overrides]]
id = ["not_stable"]
level = "information"
signal_type = "span"

[[live_check.finding_filters]]
exclude = ["deprecated"]
min_level = "improvement"
exclude_samples = ["trace.parent_id", "trace.span_id"]

# === Phase 2: CLI-equivalent settings ===

# Shared options (apply to all subcommands that accept them)
# [registry]
# path = "https://github.com/open-telemetry/semantic-conventions.git"
# follow_symlinks = false
# include_unreferenced = false
# v2 = true
#
# [policy]
# paths = ["./policies"]
# skip = false
#
# [diagnostics]
# format = "ansi"          # ansi | json | gh_workflow_command
# template = "diagnostic_templates"
# stdout = false
#
# [live_check]
# input_source = "otlp"
# input_format = "json"
# format = "ansi"
# templates = "live_check_templates"
# no_stream = false
# no_stats = false
#
# [live_check.otlp]
# grpc_address = "0.0.0.0"
# grpc_port = 4317
# admin_port = 4320
# inactivity_timeout = 10
#
# [live_check.emit]
# otlp_logs = false
# otlp_logs_endpoint = "http://localhost:4317"
# otlp_logs_stdout = false
#
# [generate]
# templates = "templates"
# target = ""
#
# [emit]
# endpoint = "http://localhost:4317"
```

## Why Two Files / Two Formats?

- **YAML for packages** — template config is complex (nested structures, comment format definitions, glob patterns). YAML is already the established format for this.
- **TOML for projects** — project config is flatter and simpler. TOML is the standard for project-level config in Rust ecosystems, and the dotfile convention (`.weaver.toml`) signals "project config, check this in."
- Two distinct files with distinct formats makes it impossible to confuse one for the other.

## Implementation Phases

**Phase 1:** Introduce `.weaver.toml` with `[live_check]` finding overrides and filters.

**Phase 2:** Add other CLI-equivalent settings to `.weaver.toml`.

**Phase 3:** Rename `weaver.yaml` to `template_config.yaml`. Load from the package directory only — remove directory-walking and `$HOME` fallback. Accept `weaver.yaml` as a deprecated fallback with a warning.

**Phase 4:** Remove `weaver.yaml` fallback.

## Summary

|                | Package Config                             | Project Config                               |
| -------------- | ------------------------------------------ | -------------------------------------------- |
| **File**       | `template_config.yaml` (was `weaver.yaml`) | `.weaver.toml`                               |
| **Belongs to** | Template package                           | Project repository                           |
| **Written by** | Package author                             | Project developer                            |
| **Discovery**  | Template dir root only                     | CWD walking + `--config`                     |
| **Contains**   | Template engine settings, default params   | CLI settings, finding mods, project defaults |
| **Format**     | YAML                                       | TOML                                         |
| **Crate**      | `weaver_forge`                             | `weaver_config`                              |
