# Default Templates

Status: Proposal

## Introduction

This document outlines how Weaver can enable federated development and inclusion of out of the box templates and policies for code generation, documentation, error reporting and stability enforcement.

## Background

Today, weaver provides a set of extension points via:

- Policies: rego files
- Templates: jinja and jq via `weaver.yaml`

These are located in the `defaults` director and are loaded into weaver's binary via `include*!` macros within rust.

While this gives us a baseline mechanism to offer a "batteries-loaded" weaver experience, it suffers from the following:

* It bloats the binary size of weaver by including all templates in the binary.
* It requires all "out of the box" capabilities be submitted to weaver's core code.
* Only *some* mechanisms allow the user to override behavior. For example, our `defaults/jq` files are loaded for *all* JQ expressions.

Weaver also provides extension mechanisms for these capabilities:

* Policies are loaded as raw `*.rego` files next to definition schemas. These will be enforced based on what `package` they declare.
* Templates are loaded from template directories (those with a `weaver.yaml` file). This can reference *remote* directories via the same "virtual directory reference", e.g. `--template https://github.com/open-telemetry/opentelemetry-weaver-templates.git\java`

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

## Design Proposal

The design relies on three key capabilities:

- Expanding weaver's `VirtualDirectory` capabilities to support additional verification features on remote directories (archive downloads and git repositories).
- Creation of a `opentelemetry-weaver-templates` repository for contributors to out of the box templates for weaver.
- Updating weaver's release process to allow pulling in default templates.
  - Creation of a release configuration file that determines which templates, policies and JQ helpers will be loaded by default in weaver.
  - Updating the weaver build process to grab (and cache?) these templates.
  - Including a manifest in weaver's release of which template versions are included.

### Fundamental Capabilities

We expand weaver `TemplateDirectoryRef` resolution to include
new security / verification features.

- ZIP - need the ability to verify signatures.
- GIT - need the ability to reference specific commits.

### Weaver Templates Repository

- Creation of weaver templates repository
- Each directory is a theme/name addressable distro 
    - `codegen/java`, `codegen/go`, etc.
    - `docs/markdown`, `docs/html`, etc.
    - `checks/semconv`, `checks/backwards-compatibility`, etc.
    - `diagnostics/ansi`, `diagnostics/gh-action`
- Each theme/name has different set of codeowners.

### Weaver Release Process

First, we create a configuration file for weaver for what to include in each release:

```yaml
templates:
  codegen:
    java: https://github.com/open-telemetry/opentelemetry-weaver-templates.git:<sha>\codegen/java
  docs:
    markdown: https://github.com/open-telemetry/opentelemetry-weaver-templates.git:<sha>\docs/markdown
policies:
    checks:
      semconv: https://github.com/open-telemetry/opentelemetry-weaver-templates.git:<sha>\checks/semconv
    advice:
      semconv: https://github.com/open-telemetry/opentelemetry-weaver-templates.git:<sha>\advice/semconv
jq:
  template:
    - defaults/jq/semconv.jq
  advice:
    - defaults/jq/advice.jq
```

Ideally this document could be kept up-to-date by `rennovate` or some other dependency bot over time.

Next, we create a `weaver_defualts` crate that is responsible for having the contents of this configuration file available to weaver.  This would resolve the config file *at build time*
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
