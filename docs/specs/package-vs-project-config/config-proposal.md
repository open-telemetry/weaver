# Proposal: Package Config vs Project Config

## Background

Weaver already has configuration files that sit alongside the artifacts they describe. A `manifest.yaml` lives at the root of a semantic convention registry and declares its name, version, and dependencies. A `weaver.yaml` lives at the root of a template package and defines how those templates are processed — syntax delimiters, comment formats, text maps, default params, and application modes.

This pattern — config adjacent to artifacts — works well because it keeps configuration close to the thing it configures. The config belongs to the package: it's authored, versioned, and distributed together with the artifacts.

## Problem

Today `weaver.yaml` is the only configuration mechanism, and it's being asked to serve two roles. Beyond configuring the template package it sits in, it also supports directory-walking and a `$HOME/.weaver/weaver.yaml` fallback — features designed to let users customize behavior from outside the package. This conflation creates confusion:

- A `weaver.yaml` in a parent directory or `$HOME` silently merges into every template package loaded below it. It's unclear where a setting originates.
- New features like live-check finding modification don't belong in template package config, but there's nowhere else to put project-level concerns.

## Proposal

### 1. Rename `weaver.yaml` to `weaver_template.yaml`

The name `weaver.yaml` is too generic — it sounds like it configures Weaver itself, when it actually configures a template package. Renaming to `weaver_template.yaml` makes its purpose explicit: it is the configuration for the templates it sits alongside.

`weaver.yaml` will continue to be loaded as a fallback during a deprecation period, with a warning directing package authors to rename to `weaver_template.yaml`.

### 2. Load template config from the package directory only

`weaver_template.yaml` is read from the given template directory and nowhere else. No directory-walking. No `$HOME/.weaver/` fallback. This is already the behavior for remote template packages and the update-markdown command — we are making local packages consistent with remote ones.

A template package should be self-contained and predictable. The same package produces the same output regardless of what `weaver.yaml` files happen to exist in parent directories or the user's home directory.

The `--config` / `-c` flag on `generate` (which currently accepts additional `weaver.yaml` files for layering) is deprecated as part of this change. It was the explicit version of the same layering mechanism — with `.weaver.toml` taking over project-level overrides, it is no longer needed.

### 3. Introduce `.weaver.toml` as project config

A new `.weaver.toml` file belongs to the **project** — the codebase that uses Weaver. A project may be a single repository or a monorepo containing multiple services. It configures everything that does not belong to a template package: settings that are CLI-configured today, plus new project-level settings that are cumbersome to provide on the command line.

**Discovery strategy — open question:** `.weaver.toml` is discovered by walking up from CWD through parent directories to root and then includes `$HOME/.weaver.toml`. Two approaches:

- **First match wins:** Use the nearest `.weaver.toml` found. Simple — you always know which file is in effect. A service overrides the root entirely.
- **Walk and merge:** Collect all `.weaver.toml` files and merge them, nearest taking precedence. This is the approach [Cargo uses](https://doc.rust-lang.org/cargo/reference/config.html#hierarchical-structure). A monorepo defines shared defaults at the root while services override specific settings.

```
$HOME/.weaver.toml              # user-wide defaults
monorepo/
├── .weaver.toml                # shared project defaults
├── service-a/
│   ├── .weaver.toml            # service-a overrides
│   └── ...
└── service-b/
    └── ...                     # inherits monorepo defaults (no local .weaver.toml)
```

With **first match**, running from `service-a/` uses only `service-a/.weaver.toml` and ignores the rest. With **walk and merge**, all three files are merged with `service-a/.weaver.toml` winning on conflicts.

Settings fall into two categories: **persistent** settings that define how the project uses Weaver (e.g. finding filters, policy paths) and **per-invocation** settings that vary between runs (e.g. input source, output format). Some settings may be available in both the config file and on the CLI, with CLI flags taking precedence. The decision about whether a given setting belongs in the config file, the CLI, or both should be made on a case-by-case basis as each setting is implemented.

**What it configures:**

- Live-check finding filters
- Live-check OTLP and emit settings
- Template defaults — shared settings like `acronyms`, `template_syntax`, `whitespace_control`, and `params` that apply across all template packages used by the project, overriding the package's own `weaver_template.yaml` defaults
- Default registry path, template path, policies path
- Any other setting that is currently a CLI flag

**Example** (illustrative — specific settings and naming may change as they are implemented):

```toml
# === Phase 1: Live-check finding filters ===

[[live_check.finding_filters]]
exclude = ["deprecated"]
min_level = "improvement"
exclude_samples = ["trace.parent_id", "trace.span_id"]

# === Template defaults ===
# Shared settings applied on top of all template packages.
# These override the package's own weaver_template.yaml.
#
# [template_defaults]
# acronyms = ["API", "HTTP", "SDK", "CLI", "URL", "JSON"]
#
# [template_defaults.template_syntax]
# block_start = "{%"
# block_end = "%}"
# variable_start = "{{"
# variable_end = "}}"
# comment_start = "{#"
# comment_end = "#}"
#
# [template_defaults.whitespace_control]
# trim_blocks = true
# lstrip_blocks = true
# keep_trailing_newline = false
#
# [template_defaults.params]
# copyright_owner = "Acme"
# year = 2026

# === CLI-equivalent settings ===

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

**Phase 1:** Introduce `.weaver.toml` covering all `live-check` CLI flags plus `[[live_check.finding_filters]]`.

**Phase 2:** Add the remaining CLI-equivalent settings (`[registry]`, `[policy]`, `[diagnostics]`, `[generate]`, `[emit]`, `[template_defaults]`) to `.weaver.toml`.

**Phase 3:** Rename `weaver.yaml` to `weaver_template.yaml`. Load from the package directory only — remove directory-walking and `$HOME` fallback. Accept `weaver.yaml` as a deprecated fallback with a warning.

**Phase 4:** Remove `weaver.yaml` fallback.

## Summary

|                | Package Config                             | Project Config                               |
| -------------- | ------------------------------------------ | -------------------------------------------- |
| **File**       | `weaver_template.yaml` (was `weaver.yaml`) | `.weaver.toml`                               |
| **Belongs to** | Template package                           | Project repository                           |
| **Written by** | Package author                             | Project developer                            |
| **Discovery**  | Template dir root only                     | CWD walking                                  |
| **Contains**   | Template engine settings, default params   | CLI settings, finding mods, project defaults |
| **Format**     | YAML                                       | TOML                                         |
| **Crate**      | `weaver_forge`                             | `weaver_config`                              |
