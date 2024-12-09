# Changelog

All notable changes to this project will be documented in this file.

## [Next] - YYYY-MM-DD

What's changed

* 

## [0.12.0] - 2024-12-09

What's changed

* Issue [#502](https://github.com/open-telemetry/weaver/issues/502) - Support stability definitions from [OTEP 232](https://github.com/open-telemetry/oteps/blob/main/text/0232-maturity-of-otel.md) - ([#504](https://github.com/open-telemetry/weaver/pull/504) by @jerbly).
  * Stability enum now has these variants: `stable`, `development`, `deprecated`, `alpha`, `beta`, `release_candidate`
  * `unmaintained` is not supported yet.
  * `experimental` is still accepted when parsing but aliased to `development`.
  * The minijinja test, `experimental`, now returns true for any variant other than `stable`.
  * EBNF and JSON schema updated to define the new enum without the `experimental` variant.
* Issue [#301](https://github.com/open-telemetry/weaver/issues/301) - Warn against usage of `allow_custom_values`. ([#514](https://github.com/open-telemetry/weaver/pull/514) by @jerbly).
* Fixed rego typos, attrigute and deprecaded ([#517](https://github.com/open-telemetry/weaver/pull/517) by @jerbly).
* Create better HTML comment parser - Allow more semconv snippet headers ([#512](https://github.com/open-telemetry/weaver/pull/512) by @jsuereth).
* Add javadoc <p> tag after lists ([#511](https://github.com/open-telemetry/weaver/pull/511) @trask).
* Javadoc <p> tags should only precede paragraphs ([#510](https://github.com/open-telemetry/weaver/pull/510) by @trask).
* More consistent newline behavior ([#509](https://github.com/open-telemetry/weaver/pull/509) by @trask).
* Add test cases to cover a paragraph after a list ([#508](https://github.com/open-telemetry/weaver/pull/508) by @trask).

Important note: Our CI/CD pipeline has been updated to use Rust cross-compilation, significantly speeding up the ARM
target (see [#506](https://github.com/open-telemetry/weaver/pull/506, thanks to @bernot-dev). If you encounter any
issues on ARM, please let us know as soon as possible. Thank you!

## [0.11.0] - 2024-12-05

What's changed

* Detect duplicate group ids, group names, and metric names. ([#382](https://github.com/open-telemetry/weaver/pull/382) by lquerel).
* Add support for Maps `map[]` to the definition of an `AnyValue`. ([#396](https://github.com/open-telemetry/weaver/pull/396) by @MSNev).
* Update semconv schema, syntax doc and validity check to correctly define `stability` as optional for attribute groups. ([#467](https://github.com/open-telemetry/weaver/pull/467) by @jerbly).
* Fix issue [#405](https://github.com/open-telemetry/weaver/issues/405) - Updated the EBNF and JSON schema to define the `extends` or `attributes` requirement mandatory for all group types except `metric` and `event`. Added a group validity check as a warning. ([#494](https://github.com/open-telemetry/weaver/pull/494) by @jerbly).
* Allow adding a description when using opt_in requirement level ([#392](https://github.com/open-telemetry/weaver/pull/392) by @joaopgrassi)
* Add warning that issues when using prefix on groups ([#407](https://github.com/open-telemetry/weaver/pull/407) by @jsuereth)
* Update comment filter to remove trailing spaces ([#453](https://github.com/open-telemetry/weaver/pull/453) by @jsuereth)
* Metrics and Events don't require attributes ([#494](https://github.com/open-telemetry/weaver/pull/494) by @jerbly)
* Added an option to follow symbolic links when loading the registry in various parts of the codebase. ([#468](https://github.com/open-telemetry/weaver/pull/468) by @leo6leo)
* Provide max line-length in comment filter. ([#454](https://github.com/open-telemetry/weaver/pull/454) by @jsuereth)

## [0.10.0] - 2024-09-23

What's changed

* Add support log based `event` definitions with a `body` of new `AnyValue` type. ([#297](https://github.com/open-telemetry/weaver/pull/297) by @MSNev).
* Add `escape_square_brackets` into `comment_formats` markdown configuration. ([#379](https://github.com/open-telemetry/weaver/pull/379) by @lquerel).
* Add `enforce_trailing_dots` into the `comment_formats` configuration. ([#378](https://github.com/open-telemetry/weaver/pull/378) by @lquerel).
* Add support for `indent_type` in both the comment filter and the `comment_formats` configuration. ([#377](https://github.com/open-telemetry/weaver/pull/377) by @lquerel).
* Add `regex_replace` filter to support replacing text using regex. ([#380](https://github.com/open-telemetry/weaver/pull/380) by @lquerel).
* Bump opentelemetry_sdk from 0.24.1 to 0.25.0 (#369)
* Bump opentelemetry-stdout from 0.5.0 to 0.25.0 (#368)
* Bump anyhow from 1.0.88 to 1.0.89 (#370)
* Bump regorus from 0.2.4 to 0.2.5 (#375)
* Bump minijinja-contrib from 2.2.0 to 2.3.1 (#376)
* Bump minijinja from 2.2.0 to 2.3.1 (#371)
* Bump globset from 0.4.14 to 0.4.15 (#366)

## [0.9.2] - 2024-09-09

What's Changed

* Build X86 + ARM64 image ([#346](https://github.com/open-telemetry/weaver/pull/346) by jsuereth). The parameter
  `--platform=linux/x86_64` is no longer needed to run this image on MacOS.
* Update docker guide for home directory ([#356](https://github.com/open-telemetry/weaver/pull/356) by jsuereth).
* Fix clippy issues. ([#357](https://github.com/open-telemetry/weaver/pull/357) by jsuereth).
* Bump alpine from 3.20.2 to 3.20.3 (#360)
* Bump anyhow from 1.0.86 to 1.0.87 (#359)
* Bump serde from 1.0.209 to 1.0.210 (#358)
* Bump serde_json from 1.0.127 to 1.0.128 (#354)
* Bump clap from 4.5.16 to 4.5.17 (#351)
* Bump regorus from 0.2.3 to 0.2.4 (#350)
* Bump indexmap from 2.4.0 to 2.5.0 (#349)
* Bump minijinja-contrib from 2.1.2 to 2.2.0 (#347)
* Bump ratatui from 0.28.0 to 0.28.1 (#341)
* Bump flate2 from 1.0.32 to 1.0.33 (#342)
* Bump minijinja from 2.1.2 to 2.2.0 (#343)
* Bump serde from 1.0.208 to 1.0.209 (#344)
* Bump serde_json from 1.0.125 to 1.0.127 (#340)

## [0.9.1] - 2024-08-22

Fixes

* Warnings detected in the baseline registry are now ignored and non-fatal errors will not
  interrupt any command before it completes
  ([#337](https://github.com/open-telemetry/weaver/pull/337) by lquerel).

## [0.9.0] - 2024-08-19

What's Changed

* ([#309](https://github.com/open-telemetry/weaver/pull/309) by lquerel) Configurable Comment Filter to Support Multiple Programming Language Comment Formats.
  More details in [Weaver Force Doc](https://github.com/open-telemetry/weaver/blob/main/crates/weaver_forge/README.md)
  and [Weaver Configuration Doc](https://github.com/open-telemetry/weaver/blob/main/docs/weaver-config.md).
* ([#300](https://github.com/open-telemetry/weaver/pull/300) by lquerel) Validation for the examples attribute field.
* ([#322](https://github.com/open-telemetry/weaver/pull/322), [#312](https://github.com/open-telemetry/weaver/pull/312),
  [#319](https://github.com/open-telemetry/weaver/pull/319), [#318](https://github.com/open-telemetry/weaver/pull/318),
  [#312](https://github.com/open-telemetry/weaver/pull/312), [#304](https://github.com/open-telemetry/weaver/pull/304)
  by jsuereth) Many improvements have been made to the creation of the Weaver Docker image,
  which is now scoring an A on the Scout Docker image score.
    * Add Weaver docker image to dependabot tracking,
    * Add build attestations,
    * Stop using root user the docker image,
    * Use official docker action to build docker image,
    * Update docker to use release build.
* ([#311](https://github.com/open-telemetry/weaver/pull/311) by MSNev) Fix `unknown.com` test reference issue.
* ([#307](https://github.com/open-telemetry/weaver/pull/307) by lmolkova) Move semconv schema definition from build tools.
* ([#305](https://github.com/open-telemetry/weaver/pull/305) by lquerel) Detect root attribute name duplicates during the resolution process.
* ([#294](https://github.com/open-telemetry/weaver/pull/294) by lquerel) Add template-level parameters and file_name per template config.
* (#327) Bump `regorus` from 0.2.2 to 0.2.3.
* (#326, #317, #302) Bump `clap` from 4.5.13 to 4.5.16.
* (#325, #313) Bump `serde` from 1.0.205 to 1.0.208.
* (#324) Bump `alpine` from 3.18.3 to 3.20.2.
* (#323) Bump `rust` from 1.76.0-alpine3.18 to 1.78.0-alpine3.18.
* (#320, #315, #287) Bump `serde_json` from 1.0.122 to 1.0.125.
* (#316) Bump `indexmap` from 2.3.0 to 2.4.0.
* (#314, #308) Bump `markdown` from 1.0.0-alpha.18 to 1.0.0-alpha.20.
* (#310) Bump `ratatui`, `textarea` and `crossterm` version in lock-step.
* (#303, #299, #293) Bump `tui-textarea` from 0.5.2 to 0.6.1.
* (#298) Bump `ratatui` from 0.27.0 to 0.28.0.
* (#292) Bump `flate2` from 1.0.30 to 1.0.31.
* (#290) Bump `regex` from 1.10.5 to 1.10.6.
* (#286) Bump `crossterm` from 0.27.0 to 0.28.1.

## [0.8.0] - 2024-08-01

What's Changed

* (#257 by lquerel) Infrastructure to support backward-compatibility testing and, more generally, policies applied to multi-version registries.

```
weaver registry check \
--registry https://github.com/open-telemetry/semantic-conventions.git[model] \
--baseline-registry https://github.com/open-telemetry/semantic-conventions/archive/refs/tags/v1.26.0.zip[model] \
--policy compatibility_check.rego
```

* (#284 by MadVikingGod) The `--policy` flag now accepts directories.
* (#270 by @lquerel) Follow build tools’ case conversion rules. Numbers are no longer considered word splitters.
* (#276 by @jsuereth) Remove legacy way of writing templates for semconv.
* (#274 by @lquerel) Enhance error reporting for invalid JQ expressions.
* (#275 by @lquerel) The custom JQ filter semconv_grouped_metrics now sorts metrics by their metric_name (issue #268).
* (#256) Bump gix from 0.63.0 to 0.64.0.
* (#271) Bump jaq-parse from 1.0.2 to 1.0.3.
* (#272) Bump jaq-core from 1.5.0 to 1.5.1
* (#273) Bump toml from 0.8.16 to 0.8.17
* (#283) Bump minijinja from 2.1.0 to 2.1.1

## [0.7.0] - 2024-07-22

What's Changed

* Add support for new custom semconv JQ filters by @lquerel.
* Update Weaver Forge documentation and include a step-by-step guide for codegen authors by @lquerel.

The following new filters have been added to the Weaver Forge:

* `semconv_group_attributes_by_root_namespace`: Groups the attributes by their root namespace.
* `semconv_attributes($options)`: Extracts and processes semantic convention attributes based on provided options. $options is an object that can contain:
  * `exclude_stability`: a list of stability statuses to exclude.
  * `exclude_deprecated`: a boolean to exclude deprecated metrics.
  * `exclude_root_namespace`: a list of root namespaces to exclude.
* `semconv_attributes`: Convenience function to extract all attributes without any filtering options.
* `semconv_grouped_attributes($options)`: Groups the processed attributes by their root namespace based on provided options. $options is an object that can contain:
  * `exclude_stability`: a list of stability statuses to exclude.
  * `exclude_deprecated`: a boolean to exclude deprecated metrics.
  * `exclude_root_namespace`: a list of root namespaces to exclude.
* `semconv_grouped_attributes`: Convenience function to group all attributes by their root namespace without any filtering options.
* `semconv_group_metrics_by_root_namespace`: Groups the metrics by their root namespace.
* `semconv_metrics($options)`: Extracts and processes semantic convention metrics based on provided options. $options is an object that can contain:
  * `exclude_stability`: a list of stability statuses to exclude.
  * `exclude_deprecated`: a boolean to exclude deprecated metrics.
  * `exclude_root_namespace`: a list of root namespaces to exclude.
* `semconv_metrics`: Convenience function to extract all metrics without any filtering options.
* `semconv_grouped_metrics($options)`: Groups the processed metrics by their root namespace based on provided options. $options is an object that can contain:
  * `exclude_stability`: a list of stability statuses to exclude.
  * `exclude_deprecated`: a boolean to exclude deprecated metrics.
  * `exclude_root_namespace`: a list of root namespaces to exclude.
* `semconv_grouped_metrics`: Convenience function to group all metrics by their root namespace without any filtering options.

## [0.6.0] - 2024-07-16

What's Changed

* Support for Hierarchical Weaver Config: We have added support for hierarchical configuration in Weaver. 
This allows more flexible and powerful configuration management. For more details, please refer to the
documentation on [configuration file loading order and overriding rules](https://github.com/open-telemetry/weaver/blob/main/docs/weaver-config.md#configuration-file-loading-order-and-overriding-rules). by @lquerel in https://github.com/open-telemetry/weaver/pull/231
* Support for MiniJinja py_compat Extensions: This release includes support for MiniJinja py_compat
extensions, enhancing compatibility with Python syntax. For more information, see the [documentation](https://github.com/open-telemetry/weaver/blob/main/crates/weaver_forge/README.md#jinja-filters). by @lquerel
in https://github.com/open-telemetry/weaver/pull/239

## New Contributors

* @haidong made a first contribution in https://github.com/open-telemetry/weaver/pull/237

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