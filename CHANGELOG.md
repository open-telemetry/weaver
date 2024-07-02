# Changelog

All notable changes to this project will be documented in this file.

## [0.5.0] - 2024-07-02

What's Changed

* Add optional variant to requirement_level. by @MadVikingGod in https://github.com/open-telemetry/weaver/pull/199
* Add semconv_const filter to support semantic convention namespacing rules. by @lquerel in https://github.com/open-telemetry/weaver/pull/200
* Add display_name field. by @joaopgrassi in https://github.com/open-telemetry/weaver/pull/202
* Bump regex from 1.10.4 to 1.10.5 by @dependabot in https://github.com/open-telemetry/weaver/pull/205
* Bump clap from 4.5.6 to 4.5.7 by @dependabot in https://github.com/open-telemetry/weaver/pull/206
* New entry in developer guide to describe the process of adding new fields in the semantic convention registry by @lquerel in https://github.com/open-telemetry/weaver/pull/209
* Add Embed option for single attributes by @trisch-me in https://github.com/open-telemetry/weaver/pull/212
* Bump include_dir from 0.7.3 to 0.7.4 by @dependabot in https://github.com/open-telemetry/weaver/pull/213
* Add support for post-resolution policies by @lquerel in https://github.com/open-telemetry/weaver/pull/214
* split_id filter is singular by @bryannaegele in https://github.com/open-telemetry/weaver/pull/217
* Add Jinja whitespace control by @joaopgrassi in https://github.com/open-telemetry/weaver/pull/224

## New Contributors
* @MadVikingGod made their first contribution in https://github.com/open-telemetry/weaver/pull/199
* @joaopgrassi made their first contribution in https://github.com/open-telemetry/weaver/pull/202
* @trisch-me made their first contribution in https://github.com/open-telemetry/weaver/pull/212
* @bryannaegele made their first contribution in https://github.com/open-telemetry/weaver/pull/217

**Full Changelog**: https://github.com/open-telemetry/weaver/compare/v0.4.0...v0.5.0


## [0.4.0] - 2024-06-04

What's Changed

* First cut at a developer's guide to help onboarding users. by @jsuereth in https://github.com/open-telemetry/weaver/pull/166
* Detect and Process Policy Files into SemConv Registry + Generic Diagnostic Reporting by @lquerel in https://github.com/open-telemetry/weaver/pull/153
* Bump gix from 0.62.0 to 0.63.0 by @dependabot in https://github.com/open-telemetry/weaver/pull/170
* Update opentelemetry rust API by @lquerel in https://github.com/open-telemetry/weaver/pull/169
* Bump serde from 1.0.202 to 1.0.203 by @dependabot in https://github.com/open-telemetry/weaver/pull/176
* Support for loading templates from the file system or from an embedded representation in the app's binary. by @lquerel in https://github.com/open-telemetry/weaver/pull/171
* Add support for List of Array examples. by @jerbly in https://github.com/open-telemetry/weaver/pull/177
* Add distribution (binaries + installers) publishing workflows. by @jsuereth in https://github.com/open-telemetry/weaver/pull/179
* Generate JSON Schema for both Resolved Telemetry Schema and Resolved Registry by @lquerel in https://github.com/open-telemetry/weaver/pull/187
* Update README.md, fix Weaver checker link by @xrmx in https://github.com/open-telemetry/weaver/pull/191
* Support command line parameters to add an additional layer of configurability in the documentation/code generator. by @lquerel in https://github.com/open-telemetry/weaver/pull/195

## New Contributors
* @jerbly made their first contribution in https://github.com/open-telemetry/weaver/pull/177
* @xrmx made their first contribution in https://github.com/open-telemetry/weaver/pull/191

**Full Changelog**: https://github.com/open-telemetry/weaver/compare/v0.3.0...v0.4.0


## [0.3.0] - 2024-05-16

What's Changed

- Additional filters and tests by @lquerel in https://github.com/open-telemetry/weaver/pull/163
    - `instantiated_type`: Filters a type to return the instantiated type.
    - `enum_type`: Filters a type to return the enum type or an error if the type is not an enum.
    - `capitalize_first`: Capitalizes the first letter of a string.
    - `map_text` introduces a second parameter to define the default value if the name of the text map or the input are not found in the `text_maps` section (optional parameter).
    - `enum`: Tests if an attribute has an enum type.
    - `simple_type`: Tests if a type is a simple type (i.e.: string | string[] | int | int[] | double | double[] | boolean | boolean[]).
    - `template_type`: Tests if a type is a template type (i.e.: template[]).
    - `enum_type`: Tests if a type is an enum type.


**Full Changelog**: https://github.com/open-telemetry/weaver/compare/v0.2.0...v0.3.0

## [0.2.0] - 2024-04-26

Updates for Semantic Convention markdown generation, and beginnings of a suite of utilities for code generation.

What's Changed:

- Working rust codegen example by @lquerel in https://github.com/open-telemetry/weaver/pull/136
- Markdown snippet generation now uses weaver_forge templating by @jsuereth in https://github.com/open-telemetry/weaver/pull/141
- New Jinja filters and predicates for OTel by @lquerel in https://github.com/open-telemetry/weaver/pull/143
- `attribute_sort` filter to weaver_forge by @jsuereth in https://github.com/open-telemetry/weaver/pull/144
- Expanding collection of filters by @lquerel in https://github.com/open-telemetry/weaver/pull/162
- (chore) Removal of Old Tera Templates by @lquerel in https://github.com/open-telemetry/weaver/pull/145
- (fix) Expand id parsing by @jsuereth in https://github.com/open-telemetry/weaver/pull/152
- (fix) Update weaver to understand deprecated enum values. by @jsuereth in https://github.com/open-telemetry/weaver/pull/139

**Full Changelog**: https://github.com/open-telemetry/weaver/compare/v0.1.0...v0.2.0

## [0.1.0] - 2024-04-24

Initial release of OpenTelemetry weaver for usage in semantic-conventions repository.

This is a PREVIEW release, and stability guarantees are loose prior to 1.0.

What's Changed:

- The Weaver project, initially hosted by F5, has been moved to open-telemetry/weaver. The project's objectives have
been redefined into two main phases/focuses: 1) semconv support, 2) application telemetry support. 
- A Jinja-compatible template engine and a snippet-based generator have been completed and tested to support the
semantic-convention repository. The template engine can be used for both documentation and code generation.
- A new policy engine (based on rego) has been added to the project to externalize the declaration of policies and to
enhance the management, evolution, and maintainability of semantic conventions and application telemetry schemas. It leverages a set of rules or policies to ensure the coherence and quality of these conventions and schemas over time.
- A lot of documentation has been added to the entire project to make it easier to consume and contribute.
- A code coverage process has been implemented with the initial goal of keeping the project above 70% coverage.
- A process for cleaning up APIs has been initiated in anticipation of publishing the crates on crates.io. The
weaver_semconv crate is the first to undergo this process.

## [unreleased]

### 🚀 Features

- *(registry)* Improve resolved schema and registry api usability.
- *(registry)* Introduce the concept of named registries
- *(stats)* Implement registry stats command
- *(resolve)* Implement registry resolve command
- *(template)* Add a more complex example generating markdown files per group prefix
- *(template)* Reimplement template generation based on minijinja + jaq (jq-like filters)
- *(cli)* Add quiet mode
- *(generator)* Add support for all group types
- *(generator)* Add jq-like filter support to make artifact generation more flexible
- *(generator)* Complete the weaver registry generate command.
- *(cli)* Add update-markdown sub-command and align sub-command args in the registry command.
- *(registry)* Improve unit test to check the generated markdown
- *(registry)* Add unit test to check the generated markdown
- *(registry)* Generate markdown from jinja2 templates
- *(template)* Generate markdown files describing a registry
- *(template)* Add template syntax configuration
- *(template)* Initialize template engine with a root directory to support include clause.
- *(template)* Expose template.set_file_name method to dynamically define the file name of the output.
- *(template)* Generate registry from templates
- *(resolve)* Improve error reporting
- *(resolve)* Fix typo
- *(resolve)* Implement `include` constraint
- *(resolve)* Check `any_of` constraints
- *(template)* Integrate with minininja
- *(template)* Start integration of the case converter
- *(template)* Replace tera with minijinja to improve error handling
- *(registry)* Refactor registry sub-commands.
- *(registry)* Add `weaver check registry` command
- *(resolver)* Simplify semantic convention registry resolution function

### 🐛 Bug Fixes

- *(resolution)* Adjust other unit tests to take into account the fix
- *(resolution)* Make resolution process easy to test in unit tests
- *(resolution)* Fix resolution order
- *(resolution)* Create minimal example reproducing the bug

### 📚 Documentation

- *(template)* Add documentation to describe the template engine.
- Describe crates layout and add README.md files for every crates in the workspace.
- Clean up README.md

### 🧪 Testing

- *(integration)* Create integration test to check parsing and resolution of the official semconv repo.

### ⚙️ Miscellaneous Tasks

- *(coverage)* Improve test coverage
- *(coverage)* Remove xtask and main command line from the code coverage
- *(coverage)* Apply `tarpaulin` coverage to the entire workspace
- *(install)* Add `cargo tarpaulin` in the list of tools to install
- *(build)* Trigger ci.yml workflow for all push and pull request
- *(coverage)* Add test code coverage with cargo tarpaulin
- *(clippy)* Add more clippy lints
- *(clippy)* Fix more clippy issues
- *(clippy)* Fix explicit_into_iter_loop clippy issue
- *(git)* Make the output dir invisible for git
- *(changelog)* Add git cliff configuration
- *(code)* Make error enums non-exhaustive
- *(code)* Implement #54
- *(code)* Fix str_to_string clippy lint issues
- *(code)* Implement #54 + new clippy lint rule
- *(build)* Fix doc lint issue
- *(build)* Fix GH action
- *(build)* Add xtask
- *(build)* Replace script/check_workspace with cargo xtask validate
- *(build)* Define lint rules globally from the cargo workspace
- *(build)* Clippy lint rules to remove unwrap and enforce must_use when needed
- *(build)* Fix clippy issues
- *(doc)* Update README.md to describe check and generate sub-commands
- *(build)* Fix clippy issue
- *(build)* Fix merge issue.
- *(build)* Update cargo lock
- *(compatibility)* Align attribute type and examples definitions
- *(compatibility)* Align requirement level definition
- *(compatibility)* Align stability definition
- *(compatibility)* Make resolved registry compatible with official registry syntax
- *(clippy)* Fix clippy lint issues
- *(error)* Improve compound error management
- *(ci)* Fix toolchain version issue
- *(ci)* Attempt to fix toolchain version issue
- *(build)* Fix ci workflow
- *(build)* Fix scripts path
- *(build)* Remove allowed-external-types.toml files from the Typos control.
- *(build)* Add control procedures for workspace and public API policies
- *(build)* Run build and test only with ubuntu target for now.
- *(build)* Remove macos target for the build (API rate limit reached, we need to figure out that later).
- Add cargo lock file.
- *(dep)* Bump dependency versions
- Migrate f5/otel-weaver repo to open-telemetry/weaver repo