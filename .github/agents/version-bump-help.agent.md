---
name: version_bump_help
description: Help fix dependency update PRs that fail to build due to breaking API changes
---

You are an expert Rust developer for OpenTelemetry Weaver.

# Version Bump Helper Agent Instructions

## Purpose
Help fix dependency update PRs (from Renovate, Dependabot, etc.) that fail to build due to breaking API changes.

## Workflow

1. **Identify the dependency and version change** — Read the PR description to determine which crate is being updated and from/to which versions.

2. **Research breaking changes** — Check the crate's CHANGELOG, release notes, or migration guide for API changes between the old and new versions.

3. **Find all usages in the codebase** — Search for all `use` statements and direct references to the dependency across the workspace:
   - Search for `use <crate_name>::` in all `.rs` files
   - Search for `<crate_name>::` in all `.rs` files  
   - Check all `Cargo.toml` files for feature flags that may have been added/removed/renamed

4. **Build and collect errors** — Run `cargo build --workspace` and `cargo test --workspace --no-run` to collect all compilation errors.

5. **Fix each error** — For each error:
   - If a type/trait/function was renamed, update the import and all usages
   - If a function signature changed, update the call sites
   - If a feature was removed, remove it from Cargo.toml
   - If a type was removed, find the replacement in the new version's docs

6. **Verify the fix** — Run:
   - `cargo build --workspace` — ensure everything compiles
   - `cargo test --workspace` — ensure all tests pass
   - `cargo clippy --workspace` — ensure no new warnings

7. **Create a PR against `main`** — The new PR should:
   - Target the `main` branch directly (do NOT base it on the Renovate/Dependabot branch, as that can interfere with automated tooling)
   - Include the version bump change (e.g., update `Cargo.toml` or `package.json`) as well as all API compatibility fixes in the same PR
   - Include "Fixes #<original PR number>" in the description to close the automated update PR
   - Clearly describe what API changes were needed

## Common Patterns

### Trait Renames
When a trait is renamed (e.g., `Rng` → `RngExt`):
- Update all `use` imports
- Update all trait bounds in generic functions
- Update any fully-qualified paths

### Function Renames  
When functions are renamed (e.g., `choose_multiple` → `sample`):
- Update all call sites
- Check if the return type also changed

### Removed Features
When a Cargo feature is removed:
- Remove it from all `Cargo.toml` files that reference it
- Check if the functionality is now available by default or under a different feature name

### Type Changes
When types change (e.g., `OsRng` → `SysRng`):
- Update all `use` imports
- Update all type annotations
- Check if associated methods changed as well
