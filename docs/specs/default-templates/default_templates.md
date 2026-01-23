# Default Templates

Status: Proposal

## Introduction

This document outlines how Weaver can enable federated development and inclusion of out of the box templates and policies for code generation, documentation, error reporting and stability enforcement.

## Background

Today, weaver provides a set of extension points via:

- Policies: rego files
- Templates: jinja and jq via `weaver.yaml`

These are located in the `defaults` directory and are loaded into weaver's binary via `include*!` macros within rust.

While this gives us a baseline mechanism to offer a "batteries-loaded" weaver experience, it suffers from the following:

* It bloats the binary size of weaver by including all templates in the binary.
* It requires all "out of the box" capabilities be submitted to weaver's core code.
* Only *some* mechanisms allow the user to override behavior. For example, our `defaults/jq` files are loaded for *all* JQ expressions.

Weaver also provides extension mechanisms for these capabilities:

* Policies are loaded as raw `*.rego` files next to definition schemas. These will be enforced based on what `package` they declare.
* Templates are loaded from template directories (those with a `weaver.yaml` file). This can reference *remote* directories via the same "virtual directory reference", e.g. `--template https://github.com/open-telemetry/opentelemetry-weaver-packages.git\codegen/java`

## Requirments & Goals

We'd like to update weaver to allow the following:

- Language, Policy and Documentation experts are able to contribute templates and policies independently of the main contribution of weaver.
    - They can depend on a *stable* verion of weaver for development
    - They can independently verify/test their templates.
    - They can independently release their templates.
- Weaver can decide which versions of templates and policies to
  include by default in any release.
    - Weaver can depend on *stable* versions of templates & policies
    - Weaver can `smoke test` that its inclusion is working.
    - Weaver can (optionally) bundle templates with its executable or docker container to avoid the need for network connection for out of the box behavior.
- Out of the box templates and policies can be referred to via "simple" names, e.g. how `ansi` works for diagnostic output today.
- Weaver users are able to override out of the box defaults.

## Example Usage

This is an intended "use" for default templates in weaver.

**weaver check**
```bash
weaver registry check \
  -r my_registry
  -p telemetry/semconv-style
```

**weaver generate**
```bash
weaver registry generate \
  -r my_registry
  --template code/java
```

**weaver live-check**
```bash
weaver registry live-check \
  -r my_registry
  -p live-check/force-schemas
```

## Design Proposal

The design relies on three key capabilities:

- Expanding weaver's `VirtualDirectory` capabilities to support additional verification features on remote directories (archive downloads and git repositories).
- Creation of a `opentelemetry-weaver-packages` repository for contributors to out of the box extensions for weaver.
- Updating weaver's release process to allow pulling in default templates.
  - Creation of a release configuration file that determines which templates, policies and JQ helpers will be loaded by default in weaver.
  - Updating the weaver build process to grab (and cache?) these templates.
  - Including a manifest in weaver's release of which template versions are included.

### Fundamental Capabilities

We expand weaver `TemplateDirectoryRef` resolution to include
new security / verification features.

- ZIP - need the ability to verify signatures.
- GIT - need the ability to reference specific commits.

Additionally `weaver registry live-check` uses `-p` to represent a port to bind to vs. `-p` representing policies. This discrepency will be sorted out.

### Weaver Templates Repository

- Creation of weaver templates repository
- Each directory is a theme/name addressable distro 
    - `code/java`, `code/go`, etc.
    - `docs/markdown`, `docs/html`, etc.
    - `live-check/semconv`, `telemetry/backwards-compatibility`, etc.
- Each theme/name has different set of codeowners.
- Initial set of top level directories:
  - `templates`
    - `docs`: Documentation generation
    - `code`: Code generation for various languages.
  - `policies`
    - `live-check`: Policies to apply during `weaver registry live-check`.
    - `telemetry`: Policies to apply during `weaver registry resolve|generate|check`

Note: Initially policy packages can be combined, so we prefer "light-weight" or "small and composable" packages vs. large ones.
For example, we envision something usage like:

```bash
weaver registry check \
  -r my_repo \
  --policies=telemetry/backwards-compatibility \
  --policies=telemetry/semantic-conventions-style
```

### Weaver Release Process

First, we create a configuration file for weaver for what to include in each release, e.g.

```yaml
templates:
  codegen:
    java: https://github.com/open-telemetry/opentelemetry-weaver-packages.git:<sha>\codegen/java
  docs:
    markdown: https://github.com/open-telemetry/opentelemetry-weaver-packages.git:<sha>\docs/markdown
policies:
    check:
      semconv: https://github.com/open-telemetry/opentelemetry-weaver-packages.git:<sha>\check/semconv
    live-check:
      semconv: https://github.com/open-telemetry/opentelemetry-weaver-packages.git:<sha>\live-check/semconv
jq:
  template:
    - defaults/jq/semconv.jq
  advice:
    - defaults/jq/advice.jq
```

Ideally this document could be kept up-to-date by `rennovate` or some other dependency bot over time.

Next, we create a `weaver_defaults` crate that is responsible for having the contents of this configuration file available to weaver.  This would resolve the config file *at build time*
and offer an API to access the defaults. 

This API should:
- Hide whether or not a default is loaded remotely or from
  some cache/store in/near the binary.
- Provide "built in" name resolution.

```rust
//! Functions to interact with out-of-the-box defaults in weaver.

/// Add default policies to weaver
fn add_default_policies(engine: &mut weaver_checker::Engine) -> Result<(), Error>;
/// Resolves a target string into a template engine.
/// Returns None if the target is not a built-in template.
fn resolve_template_root(target: &str) -> Result<Option<TemplateEngine>, Error>;
/// Returns any default JQ packages that should be included
/// in JQ expressions.
fn default_jq_packages(usecase: JqUseCase) -> Result<JqPackage, Error>;

/// Weaver use cases for JQ.
/// 
/// This allows pulling different defaults for different areas in weaver.
enum JqUseCase {
    Template,
    Advice
}

```

## Security Considerations

Loading dynamic configuration is a vector of attack, and concern for security. Weaver needs to follow best practices (e.g. [SLSA](https://slsa.dev/)) to avoid supply chain attacks here.

- Weaver should allow signature verification of packages it downloads as ZIP.
- Weaver should use git HASH when resolving from git repositories.

## Survey of Weaver's current built-ins

| Package | Action |
|--|--|
| `diagnostic_templates/ansi` | stays in weaver |
| `diagnostic_templates/gh_workflow_command` | stays in weaver |
| `diagnostic_templates/json` | stays in weaver* |
| `diff_templates/json` | stays in weaver* |
| `diff_templates/ansi` | stays in weaver |
| `diff_templates/ansi-stats` | stays in weaver |
| `diff_templates/markdown` | removed* |
| `diff_templates/yaml` | removed* |
| `jq/advice.jq` | stays in weaver |
| `jq/semconv.jq` | stays in weaver |
| `live_check_templates/ansi` | stays in weaver |
| `live_check_templates/json` | stays in weaver* |
| `policies/live_check_advice` | Moves to `live_check/semconv`. Enforces semconv naming. |
| `defaults/rego` | Removed when V2 syntax becomes default. |

_*Note: Raw output formats like JSON, YAML, etc. supported by SERDE will move away from using JINJA extensions and be first class export formats over time_
